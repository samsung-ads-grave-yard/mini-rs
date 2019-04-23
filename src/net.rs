use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::{CStr, CString};
use std::io;
use std::io::{
    ErrorKind,
    Read,
    Write,
};
use std::mem;
use std::net::{self, TcpStream};
use std::os::unix::io::{
    AsRawFd,
    RawFd,
};
use std::ptr;
use std::rc::Rc;
use std::str;

use async::{self, Mode};
use async::ffi::epoll_event;
use handler::{
    Loop,
    Handler,
    Stream,
};

use self::ListenerMsg::*;

#[repr(u32)]
enum StatusMode {
    Error = async::ffi::EPOLLERR,
    HangupError = async::ffi::EPOLLHUP,
}

/*fn get_nonblocking<A: AsRawFd>(socket: &A) -> io::Result<bool> {
    let val = unsafe { ffi::fcntl(socket.as_raw_fd(), ffi::F_GETFL, 0) };
    if val < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(val & ffi::O_NONBLOCK != 0)
}*/

// TODO: move this function elsewhere?
pub fn set_nonblocking<A: AsRawFd>(socket: &A) -> io::Result<()> {
    let val = unsafe { ffi::fcntl(socket.as_raw_fd(), ffi::F_SETFL, ffi::O_NONBLOCK) };
    if val < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub mod tcp {
    use std::mem;
    use std::net::TcpStream;
    use std::os::unix::io::{
        AsRawFd,
        FromRawFd,
    };
    use std::marker::PhantomData;

    use async::{
        Mode,
    };
    use async::ffi::epoll_event;
    use handler::{
        Handler,
        Loop,
        Stream,
    };
    use self::ffi::ErrNo;
    use self::Msg::*;
    use super::{
        AddrInfoIter,
        ConnectionComponentMsg,
        ConnectionMsg,
        StatusMode,
        TcpConnection,
        TcpConnectionNotify,
        close,
        connect,
        ffi,
        getaddrinfo,
        getsockopt,
        manage_connection,
        socket,
    };

    pub enum Msg<NOTIFY> {
        TryingConnectionToHost(NOTIFY, AddrInfoIter, u32),
        WriteEvent(epoll_event, TcpConnection, NOTIFY, AddrInfoIter, u32),
    }

    struct Connector<NOTIFY> {
        connection_stream: Stream<ConnectionMsg>,
        _phantom: PhantomData<NOTIFY>,
    }

    impl<NOTIFY> Connector<NOTIFY> {
        fn new(connection_stream: &Stream<ConnectionMsg>) -> Self {
            Self {
                connection_stream: connection_stream.clone(),
                _phantom: PhantomData,
            }
        }
    }

    impl<NOTIFY> Handler for Connector<NOTIFY>
    where NOTIFY: TcpConnectionNotify + 'static,
    {
        type Msg = Msg<NOTIFY>;

        fn update(&mut self, event_loop: &mut Loop, stream: &Stream<Msg<NOTIFY>>, msg: Msg<NOTIFY>) {
            match msg {
                TryingConnectionToHost(mut connection_notify, mut address_infos, count) => {
                    match address_infos.next() {
                        Some(address_info) => {
                            match socket(address_info.ai_family, address_info.ai_socktype | ffi::SOCK_NONBLOCK,
                                         address_info.ai_protocol)
                            {
                                Ok(fd) => {
                                    let tcp_stream = unsafe { TcpStream::from_raw_fd(fd) };
                                    let mut connection = TcpConnection::new(tcp_stream);
                                    connection_notify.connecting(&mut connection, count);
                                    match unsafe { connect(fd, address_info.ai_addr, address_info.ai_addrlen) } {
                                        Ok(()) => {
                                            manage_connection(event_loop, connection, Box::new(connection_notify),
                                                Some(&self.connection_stream));
                                            //return ProcessContinuation::Stop;
                                        },
                                        Err(ref error) if error.raw_os_error() == Some(ErrNo::InProgress as i32) => {
                                            let result = event_loop.try_add_raw_fd_oneshot(fd, Mode::Write);
                                            match result {
                                                Ok(mut event) => {
                                                    event.set_callback(&stream,
                                                        // TODO: check if it should be count + 1.
                                                        move |event| WriteEvent(event, connection, connection_notify, address_infos, count)
                                                    );
                                                },
                                                Err(error) => connection_notify.error(error),
                                            }
                                        },
                                        Err(_) => {
                                            stream.send(TryingConnectionToHost(connection_notify, address_infos, count + 1));

                                            // Note that errors are ignored when closing a file descriptor. The
                                            // reason for this is that if an error occurs we don't actually know if
                                            // the file descriptor was closed or not, and if we retried (for
                                            // something like EINTR), we might close another valid file descriptor
                                            // opened after we closed ours.
                                            let _ = close(fd);
                                        },
                                    }
                                },
                                Err(_) => stream.send(TryingConnectionToHost(connection_notify, address_infos, count + 1)),
                            }
                        },
                        None => connection_notify.connect_failed(),
                    }
                },
                WriteEvent(event, connection, mut connection_notify, address_infos, count) => {
                    let fd = connection.as_raw_fd();
                    if (event.events & (StatusMode::HangupError as u32 | StatusMode::Error as u32)) != 0 {
                        stream.send(TryingConnectionToHost(connection_notify, address_infos, count + 1));
                    }
                    // TODO: should we check for Write when there's an error?
                    else if event.events & Mode::Write as u32 != 0 {
                        let result = getsockopt(fd, ffi::SOL_SOCKET, ffi::SO_ERROR);
                        match result {
                            Ok(value) if value != 0 => {
                                let _ = close(fd);
                                stream.send(TryingConnectionToHost(connection_notify, address_infos, count + 1));
                            },
                            Ok(_) => {
                                if let Err(error) = event_loop.remove_raw_fd(fd) {
                                    // TODO: not sure if it makes sense to report this error to the user.
                                    connection_notify.error(error);
                                }
                                manage_connection(event_loop, connection, Box::new(connection_notify), Some(&self.connection_stream));
                                // TODO: stop handler here.
                            },
                            Err(_) => {
                                let _ = close(fd);
                                stream.send(TryingConnectionToHost(connection_notify, address_infos, count + 1));
                            },
                        }
                    }
                },
            }
        }
    }

    struct Connection {
        connection: Option<Stream<ConnectionComponentMsg>>,
    }

    impl Connection {
        fn new() -> Self {
            Self {
                connection: None,
            }
        }
    }

    impl Handler for Connection {
        type Msg = ConnectionMsg;

        fn update(&mut self, _event_loop: &mut Loop, _stream: &Stream<Self::Msg>, msg: Self::Msg) {
            match msg {
                ConnectionMsg::Connected(connection) => self.connection = Some(connection),
                ConnectionMsg::Write(data) => {
                    if let Some(ref connection) = self.connection {
                        connection.send(ConnectionComponentMsg::Write(data));
                    }
                    else {
                        eprintln!("Not yet connected"); // TODO: handle error.
                    }
                },
            }
        }
    }

    pub fn connect_to_host<NOTIFY>(host: &str, port: &str, event_loop: &mut Loop, mut connection_notify: NOTIFY) -> Option<Stream<ConnectionMsg>>
    where NOTIFY: TcpConnectionNotify + 'static,
    {
        let mut hints: ffi::addrinfo = unsafe { mem::zeroed() };
        hints.ai_socktype = ffi::SOCK_STREAM as i32;
        // TODO: use getaddrinfo_a which is asynchronous. Maybe not: https://medium.com/where-the-flamingcow-roams/asynchronous-name-resolution-in-c-268ff5df3081
        match getaddrinfo(Some(host), Some(port), Some(hints)) {
            Ok(address_infos) => {
                let connection_stream = event_loop.spawn(Connection::new());
                let stream = event_loop.spawn(Connector::new(&connection_stream));
                stream.send(TryingConnectionToHost(connection_notify, address_infos, 0));
                Some(connection_stream)
            },
            Err(error) => {
                connection_notify.error(error); // FIXME: do we really want to both notify and return the error?
                None
            },
        }
    }
}

#[derive(Debug)]
pub struct AddrInfoIter {
    address_infos: *mut ffi::addrinfo,
    current_address_info: *mut ffi::addrinfo,
}

impl AddrInfoIter {
    fn new(address_infos: *mut ffi::addrinfo) -> Self {
        Self {
            address_infos,
            current_address_info: address_infos,
        }
    }
}

impl Iterator for AddrInfoIter {
    type Item = ffi::addrinfo;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_address_info.is_null() {
            return None;
        }
        let result = unsafe { ptr::read(self.current_address_info) };
        self.current_address_info = unsafe { (*self.current_address_info).ai_next };
        Some(result)
    }
}

impl Drop for AddrInfoIter {
    fn drop(&mut self) {
        unsafe { ffi::freeaddrinfo(self.address_infos) };
    }
}

pub fn close(fd: RawFd) -> io::Result<()> {
    if unsafe { ffi::close(fd) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub unsafe fn connect(socket: RawFd, address: *const ffi::sockaddr, address_len: ffi::socklen_t) -> io::Result<()> {
    if ffi::connect(socket, address, address_len) != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub fn getaddrinfo(hostname: Option<&str>, service: Option<&str>, hints: Option<ffi::addrinfo>) ->
    io::Result<AddrInfoIter>
{
    let hints = hints.as_ref().map(|hints| hints as *const _).unwrap_or_else(ptr::null);
    let mut address_infos = ptr::null_mut();
    let hostname = to_c_string(hostname)?;
    let service = to_c_string(service)?;
    let result = unsafe { ffi::getaddrinfo(hostname, service, hints, &mut address_infos) };
    unsafe {
        // Free memory.
        CString::from_raw(hostname as *mut _);
        CString::from_raw(service as *mut _);
    }
    if result == 0 {
        Ok(AddrInfoIter::new(address_infos))
    }
    else if result == ffi::EAI_SYSTEM {
        Err(io::Error::last_os_error())
    }
    else {
        let reason = unsafe {
            str::from_utf8(CStr::from_ptr(ffi::gai_strerror(result)).to_bytes()).unwrap_or("unknown error").to_string()
        };
        Err(io::Error::new(ErrorKind::Other, format!("failed to lookup address information: {}", reason)))
    }
}

pub fn socket(domain: i32, typ: i32, protocol: i32) -> io::Result<RawFd> {
    let result = unsafe { ffi::socket(domain, typ, protocol) };
    if result < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(result)
}

fn to_c_string(string: Option<&str>) -> io::Result<*const i8> {
    match string {
        Some(string) => {
            let string = CString::new(string)?;
            Ok(string.into_raw())
        },
        None => Ok(ptr::null()),
    }
}

pub fn getsockopt(socket: RawFd, level: i32, name: i32) -> io::Result<i32> {
    let mut option_value = 0i32;
    let mut option_len = mem::size_of_val(&option_value) as i32;
    let error = unsafe { ffi::getsockopt(socket, level, name, &mut option_value as *mut i32 as *mut _, &mut option_len as *mut i32) };
    if error == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(option_value)
}

struct Buffer {
    buffer: Vec<u8>,
    index: usize,
}

impl Buffer {
    fn new(buffer: Vec<u8>, index: usize) -> Self {
        Self {
            buffer,
            index,
        }
    }

    fn advance(&mut self, count: usize) {
        self.index += count;
    }

    fn exhausted(&self) -> bool {
        self.index >= self.len()
    }

    fn len(&self) -> usize {
        self.buffer.len()
    }

    fn slice(&self) -> &[u8] {
        &self.buffer[self.index..]
    }
}

pub enum ConnectionMsg {
    Connected(Stream<ConnectionComponentMsg>),
    Write(Vec<u8>),
}

pub enum ConnectionComponentMsg {
    ReadWrite(epoll_event),
    Write(Vec<u8>),
}

struct _TcpConnection {
    // TODO: should the VecDeque be bounded?
    buffers: VecDeque<Buffer>, // The system should probably reuse the buffer and keep adding to it even if the trait does not consume its data. That should be better than a Vec inside a VecDeque.
    disposed: bool,
    stream: TcpStream,
}

impl _TcpConnection {
    fn send(&mut self, event_loop: &mut Loop, connection_notify: &mut TcpConnectionNotify) {
        let mut remove_buffer = false;
        if let Some(ref mut first_buffer) = self.buffers.front_mut() {
            match self.stream.write(first_buffer.slice()) {
                Ok(written) => {
                    first_buffer.advance(written);
                    if first_buffer.exhausted() {
                        remove_buffer = true;
                    }
                },
                Err(ref error) if error.kind() == ErrorKind::WouldBlock => (),
                Err(ref error) if error.kind() == ErrorKind::Interrupted => (),
                Err(error) => {
                    connection_notify.error(error);
                    let _ = event_loop.remove_fd(&self.stream);
                    // TODO: remove the handler as well.
                },
            }
        }
        if remove_buffer {
            self.buffers.pop_front();
        }
    }
}

#[derive(Clone)]
pub struct TcpConnection {
    connection: Rc<RefCell<_TcpConnection>>,
}

impl TcpConnection {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            connection: Rc::new(RefCell::new(_TcpConnection {
                buffers: VecDeque::new(),
                disposed: false,
                stream,
            })),
        }
    }

    // TODO: in debug mode, warn if dispose is not called (to help in detecting leaks). Maybe
    // easier to just check if the difference of the number of callbacks allocation - the number of
    // callbacks deallocation is greater than 0.
    pub fn dispose(&self) {
        self.connection.borrow_mut().disposed = true;
    }

    fn disposed(&self) -> bool {
        self.connection.borrow().disposed
    }

    pub fn ip4<NOTIFY>(event_loop: &mut Loop, host: &str, port: u16, connection: NOTIFY) -> Option<Stream<ConnectionMsg>>
    where NOTIFY: TcpConnectionNotify + 'static,
    {
        tcp::connect_to_host(host, &port.to_string(), event_loop, connection)
    }

    fn read(&self, buffer: &mut [u8]) -> io::Result<usize> {
        self.connection.borrow_mut().stream.read(buffer)
    }

    fn send(&self, event_loop: &mut Loop, connection_notify: &mut TcpConnectionNotify) {
        let mut connection = self.connection.borrow_mut();
        connection.send(event_loop, connection_notify);
    }

    pub fn write(&self, buffer: Vec<u8>) -> io::Result<()> {
        let buffer_size = buffer.len();
        let mut stream = self.connection.borrow().stream.try_clone()?;
        let mut index = 0;
        while index < buffer.len() {
            // TODO: yield to avoid starvation?
            match stream.write(&buffer[index..]) {
                Err(ref error) if error.kind() == ErrorKind::WouldBlock => {
                    self.connection.borrow_mut().buffers.push_back(Buffer::new(buffer, index));
                    return Ok(());
                },
                Err(error) => return Err(error),
                Ok(written) => {
                    index += written;
                    if index >= buffer_size {
                        return Ok(());
                    }
                },
            }
        }
        Ok(())
    }
}

impl AsRawFd for TcpConnection {
    fn as_raw_fd(&self) -> RawFd {
        self.connection.borrow().stream.as_raw_fd()
    }
}

struct ConnectionComponent {
    connection: TcpConnection,
    connection_notify: Box<TcpConnectionNotify>,
}

impl ConnectionComponent {
    fn new(connection: TcpConnection, connection_notify: Box<TcpConnectionNotify>) -> Self {
        Self {
            connection,
            connection_notify,
        }
    }
}

impl Handler for ConnectionComponent {
    type Msg = ConnectionComponentMsg;

    fn update(&mut self, event_loop: &mut Loop, _stream: &Stream<Self::Msg>, msg: Self::Msg) {
        match msg {
            ConnectionComponentMsg::ReadWrite(event) => {
                let mut disposed = false;
                if (event.events & (StatusMode::HangupError as u32 | StatusMode::Error as u32)) != 0 {
                    // TODO: do we want to signal these errors to the trait?
                    // TODO: are we sure we want to remove the fd from epoll when there's an error?
                    if let Err(error) = event_loop.remove_raw_fd(self.connection.as_raw_fd()) {
                        // TODO: not sure if it makes sense to report this error to the user.
                        self.connection_notify.error(error);
                    }
                    self.connection_notify.closed(&mut self.connection); // FIXME: should it only be called for HangupError?
                    // TODO: stop.
                }
                if event.events & Mode::Read as u32 != 0 {
                    let mut buffer = vec![0; 4096];
                    match self.connection.read(&mut buffer) {
                        Err(ref error) if error.kind() == ErrorKind::WouldBlock ||
                            error.kind() == ErrorKind::Interrupted => (),
                        Ok(bytes_read) => {
                            if bytes_read > 0 {
                                buffer.truncate(bytes_read);
                                self.connection_notify.received(&mut self.connection, buffer);
                                disposed = disposed || self.connection.disposed();
                            }
                            else {
                                let _ = event_loop.remove_fd(&self.connection);
                                // TODO: remove the handler as well.
                            }
                        },
                        Err(_) => {
                            let _ = event_loop.remove_fd(&self.connection);
                            // TODO: remove the handler as well.
                        },
                    }
                }
                if event.events & Mode::Write as u32 != 0 {
                    self.connection.send(event_loop, &mut *self.connection_notify);
                }
                if disposed {
                    self.connection_notify.closed(&mut self.connection);
                    // TODO: stop
                }
            },
            ConnectionComponentMsg::Write(data) =>
                if let Err(error) = self.connection.write(data) {
                    self.connection_notify.error(error);
                    let _ = event_loop.remove_fd(&self.connection);
                    // TODO: remove the handler as well.
                },
        }
    }
}

pub trait TcpListenNotify {
    fn listening(&mut self, _listener: &net::TcpListener) {
    }

    fn not_listening(&mut self) {
    }

    fn closed(&mut self, _listener: &net::TcpListener) {
    }

    fn connected(&mut self, listener: &net::TcpListener) -> Box<TcpConnectionNotify>;

    fn error(&mut self, _error: io::Error) {
    }
}

pub trait TcpConnectionNotify {
    fn accepted(&mut self, _connection: &mut TcpConnection) {
    }

    fn connecting(&mut self, _connection: &mut TcpConnection, _count: u32) {
    }

    fn connected(&mut self, _connection: &mut TcpConnection) {
    }

    fn connect_failed(&mut self) { // TODO: Pony accepts a TcpConnection here. Not sure how we could get one, though.
    }

    fn auth_failed(&mut self, _connection: &mut TcpConnection) {
    }

    // TODO: create a new Error type instead of having to use io::ErrorKind::Other.
    fn error(&mut self, _error: io::Error) {
    }

    fn sent(&mut self, _connection: &mut TcpConnection, data: Vec<u8>) -> Vec<u8> {
        data
    }

    fn wait_for_bytes(&mut self, _connection: &mut TcpConnection, _quantity: usize) -> usize {
        0
    }

    fn received(&mut self, _connection: &mut TcpConnection, _data: Vec<u8>) {
    }

    fn closed(&mut self, _connection: &mut TcpConnection) {
    }

    fn throttled(&mut self, _connection: &mut TcpConnection) {
        // TODO: call when there is TCP backpressure.
    }

    fn unthrottled(&mut self, _connection: &mut TcpConnection) {
    }
}

pub enum ListenerMsg {
    ReadEvent(epoll_event),
}

fn manage_connection(event_loop: &mut Loop, mut connection: TcpConnection, mut connection_notify: Box<TcpConnectionNotify>,
    connection_stream: Option<&Stream<ConnectionMsg>>) {
    connection_notify.connected(&mut connection); // TODO: is this second method necessary?

    match event_loop.try_add_fd(&connection, Mode::ReadWrite) {
        Ok(event) => {
            let stream = event_loop.spawn(ConnectionComponent::new(connection, connection_notify));
            event.set_callback(&stream, ConnectionComponentMsg::ReadWrite);
            if let Some(ref connection_stream) = connection_stream {
                connection_stream.send(ConnectionMsg::Connected(stream));
            }
        },
        Err(error) => connection_notify.error(error),
    }
}

pub struct TcpListener<L> {
    listen_notify: L,
    tcp_listener: net::TcpListener,
}

impl<L> TcpListener<L> {
    pub fn new(tcp_listener: net::TcpListener, listen_notify: L) -> Self {
        Self {
            listen_notify,
            tcp_listener,
        }
    }

    pub fn ip4(event_loop: &mut Loop, host: &str, mut listen_notify: L)
        -> io::Result<Stream<ListenerMsg>>
    where L: TcpListenNotify + 'static,
    {
        let tcp_listener =
            match net::TcpListener::bind(host) {
                Ok(tcp_listener) => {
                    listen_notify.listening(&tcp_listener);
                    tcp_listener
                },
                Err(error) => {
                    listen_notify.not_listening();
                    return Err(error);
                },
            };
        tcp_listener.set_nonblocking(true)?;
        let fd = tcp_listener.as_raw_fd();
        let stream = event_loop.spawn(TcpListener::new(tcp_listener, listen_notify));
        event_loop.add_raw_fd(fd, Mode::Read, &stream, ReadEvent)?;
        Ok(stream)
    }
}

impl<L> Handler for TcpListener<L>
where L: TcpListenNotify,
{
    type Msg = ListenerMsg;

    fn update(&mut self, event_loop: &mut Loop, _stream: &Stream<Self::Msg>, msg: Self::Msg) {
        match msg {
            ReadEvent(event) => {
                if (event.events & (StatusMode::HangupError as u32 | StatusMode::Error as u32)) != 0 {
                    // TODO: do we want to signal these errors to the trait?
                    // TODO: are we sure we want to remove the fd from epoll when there's an error?
                    if let Err(error) = event_loop.remove_raw_fd(self.tcp_listener.as_raw_fd()) { // TODO: do a version of this method that takes a AsRawFd.
                        // TODO: not sure if it makes sense to report this error to the user.
                        self.listen_notify.error(error);
                    }
                    self.listen_notify.closed(&&self.tcp_listener); // FIXME: should it only be called for HangupError?
                    // TODO: remove this handler.
                }
                else if event.events & Mode::Read as u32 != 0 {
                    // TODO: accept many times?
                    match self.tcp_listener.accept() {
                        Ok((stream, _addr)) => {
                            match stream.set_nonblocking(true) {
                                Ok(()) => {
                                    let mut connection_notify = self.listen_notify.connected(&self.tcp_listener);
                                    let mut connection = TcpConnection::new(stream);
                                    connection_notify.accepted(&mut connection);
                                    manage_connection(event_loop, connection, connection_notify, None);
                                },
                                Err(error) => self.listen_notify.error(error),
                            }
                        },
                        Err(ref error) if error.kind() == ErrorKind::WouldBlock => (),
                        Err(error) => self.listen_notify.error(error),
                    }
                }
                // TODO: call listen_notify.closed().
                // TODO: have a message Dispose to stop listening.
            },
        }
    }
}

pub mod ffi {
    #![allow(non_camel_case_types)]

    use std::os::raw::c_void;

    #[repr(i32)]
    pub enum ErrNo {
        InProgress = 115,
    }

    pub const EAI_SYSTEM: i32 = -11;

    pub const F_GETFL: i32 = 3;
    pub const F_SETFL: i32 = 4;

    pub const O_NONBLOCK: i32 = 0o4000;

    pub const SOL_SOCKET: i32 = 1;
    pub const SO_ERROR: i32 = 4;

    pub const SOCK_STREAM: i32 = 1;
    pub const SOCK_DGRAM: i32 = 2;
    pub const SOCK_NONBLOCK: i32 = 0o4000;

    pub enum sockaddr {
    }

    pub type socklen_t = i32;

    #[repr(C)]
    pub struct addrinfo {
        pub ai_flags: i32,
        pub ai_family: i32,
        pub ai_socktype: i32,
        pub ai_protocol: i32,
        pub ai_addrlen: socklen_t,
        pub ai_addr: *mut sockaddr,
        pub ai_canonname: *mut i8,
        pub ai_next: *mut addrinfo,
    }

    extern "C" {
        pub fn connect(socket: i32, address: *const sockaddr, address_len: socklen_t) -> i32;

        pub fn close(fildes: i32) -> i32;

        pub fn freeaddrinfo(res: *mut addrinfo);
        pub fn getaddrinfo(node: *const i8, service: *const i8, hints: *const addrinfo, result: *mut *mut addrinfo)
            -> i32;
        pub fn gai_strerror(errcode: i32) -> *const i8;

        pub fn fcntl(fildes: i32, cmd: i32, ...) -> i32;

        pub fn getsockopt(socket: i32, level: i32, option_name: i32, option_value: *mut c_void, option_len: *mut socklen_t)
            -> i32;
        pub fn socket(domain: i32, typ: i32, protocol: i32) -> i32;
    }
}
