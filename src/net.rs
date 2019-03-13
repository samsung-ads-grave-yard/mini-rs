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
use std::str;

use actor::{
    Pid,
    ProcessContinuation,
    ProcessQueue,
};
use async::{
    EventLoop,
    Mode,
};

fn get_nonblocking<A: AsRawFd>(socket: &A) -> io::Result<bool> {
    let val = unsafe { ffi::fcntl(socket.as_raw_fd(), ffi::F_GETFL, 0) };
    if val < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(val & ffi::O_NONBLOCK != 0)
}

pub mod tcp {
    use std::io::ErrorKind;
    use std::mem;
    use std::net::TcpStream;
    use std::os::unix::io::FromRawFd;

    use actor::{
        self,
        Pid,
        ProcessContinuation,
        ProcessQueue,
        SpawnParameters,
    };
    use async::{
        EventLoop,
        Mode,
    };
    use self::ffi::ErrNo;
    use self::Msg::*;
    use super::{
        AddrInfoIter,
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

    #[derive(Debug)]
    enum Msg<CONNECTION> {
        TryingConnectionToHost(AddrInfoIter, u32, CONNECTION),
    }

    pub fn connect_to_host<CONNECTION>(host: &str, port: &str, process_queue: &ProcessQueue,
        event_loop: &EventLoop, mut connection_notify: CONNECTION)
    where CONNECTION: TcpConnectionNotify + Send + 'static,
    {
        fn send<CONNECTION>(pid: &Pid<Msg<CONNECTION>>, connection_notify: CONNECTION, address_infos: AddrInfoIter,
                            count: u32)
        where CONNECTION: TcpConnectionNotify,
        {
            if let Err(actor::Error { msg: TryingConnectionToHost(_, _, mut connection_notify), .. }) =
                pid.send_message(TryingConnectionToHost(address_infos, count, connection_notify))
            {
                connection_notify.error(ErrorKind::Other.into()); // TODO: use a new error type.
            }
        }

        let event_loop = event_loop.clone();
        let handler = move |current: &Pid<_>, msg: Option<Msg<CONNECTION>>| {
            if let Some(msg) = msg {
                match msg {
                    TryingConnectionToHost(mut address_infos, count, mut connection_notify) => {
                        match address_infos.next() {
                            Some(address_info) => {
                                match socket(address_info.ai_family, address_info.ai_socktype | ffi::SOCK_NONBLOCK,
                                                  address_info.ai_protocol)
                                {
                                    Ok(fd) => {
                                        let stream = unsafe { TcpStream::from_raw_fd(fd) };
                                        let mut connection = TcpConnection::new(stream);
                                        connection_notify.connecting(&mut connection, count);
                                        match connect(fd, address_info.ai_addr, address_info.ai_addrlen) {
                                            Ok(()) => {
                                                manage_connection(&event_loop, connection, Box::new(connection_notify));
                                                return ProcessContinuation::Stop;
                                            },
                                            Err(ref error) if error.raw_os_error() == Some(ErrNo::InProgress as i32) => {
                                                let current = current.clone();
                                                let eloop = event_loop.clone();
                                                let result = event_loop.try_add_raw_fd_oneshot(fd, Mode::Write);
                                                match result {
                                                    Ok(mut event) =>
                                                        event.set_callback(move |event| {
                                                            if event.events & Mode::Write as u32 != 0 {
                                                                let result = getsockopt(fd, ffi::SOL_SOCKET, ffi::SO_ERROR);
                                                                match result {
                                                                    Ok(value) if value != 0 => {
                                                                        let _ = close(fd);
                                                                        send(&current, connection_notify, address_infos, count + 1);
                                                                    },
                                                                    Ok(_) => {
                                                                        if let Err(error) = eloop.remove_raw_fd(fd) {
                                                                            // TODO: not sure if it makes sense to report this error to the user.
                                                                            connection_notify.error(error);
                                                                        }
                                                                        manage_connection(&eloop, connection, Box::new(connection_notify))
                                                                        // TODO: stop actor here.
                                                                    },
                                                                    Err(_) => {
                                                                        let _ = close(fd);
                                                                        send(&current, connection_notify, address_infos, count + 1);
                                                                    },
                                                                }
                                                            }
                                                        }),
                                                    Err(error) => connection_notify.error(error),
                                                }
                                            },
                                            Err(_) => {
                                                send(current, connection_notify, address_infos, count + 1);

                                                // Note that errors are ignored when closing a file descriptor. The
                                                // reason for this is that if an error occurs we don't actually know if
                                                // the file descriptor was closed or not, and if we retried (for
                                                // something like EINTR), we might close another valid file descriptor
                                                // opened after we closed ours.
                                                let _ = close(fd);
                                            },
                                        }
                                    },
                                    Err(_) => send(current, connection_notify, address_infos, count + 1),
                                }
                            },
                            None => connection_notify.connect_failed(),
                        }
                    },
                }
            }
            ProcessContinuation::WaitMessage
        };

        let mut hints: ffi::addrinfo = unsafe { mem::zeroed() };
        hints.ai_socktype = ffi::SOCK_STREAM as i32;
        // TODO: use getaddrinfo_a which is asynchronous. Maybe not: https://medium.com/where-the-flamingcow-roams/asynchronous-name-resolution-in-c-268ff5df3081
        match getaddrinfo(Some(host), Some(port), Some(hints)) {
            Ok(address_infos) => {
                let pid = process_queue.blocking_spawn(SpawnParameters {
                    handler,
                    message_capacity: 2,
                    max_message_per_cycle: 1,
                });
                send(&pid, connection_notify, address_infos, 0);
            },
            Err(error) => {
                connection_notify.error(error);
            },
        }
    }
}

#[derive(Debug)]
pub struct AddrInfoIter {
    address_infos: *mut ffi::addrinfo,
}

unsafe impl Send for AddrInfoIter {}

impl AddrInfoIter {
    fn new(address_infos: *mut ffi::addrinfo) -> Self {
        Self {
            address_infos,
        }
    }
}

impl Iterator for AddrInfoIter {
    type Item = ffi::addrinfo;

    fn next(&mut self) -> Option<Self::Item> {
        if self.address_infos.is_null() {
            return None;
        }
        let result = unsafe { ptr::read(self.address_infos) };
        self.address_infos = unsafe { (*self.address_infos).ai_next };
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

pub fn connect(socket: RawFd, address: *const ffi::sockaddr, address_len: ffi::socklen_t) -> io::Result<()> {
    if unsafe { ffi::connect(socket, address, address_len) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub fn getaddrinfo(hostname: Option<&str>, service: Option<&str>, hints: Option<ffi::addrinfo>) ->
    io::Result<AddrInfoIter>
{
    let hints = hints.as_ref().map(|hints| hints as *const _).unwrap_or_else(|| ptr::null());
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
    else {
        if result == ffi::EAI_SYSTEM {
            Err(io::Error::last_os_error())
        }
        else {
            let reason = unsafe {
                str::from_utf8(CStr::from_ptr(ffi::gai_strerror(result)).to_bytes()).unwrap_or("unknown error").to_string()
            };
            Err(io::Error::new(ErrorKind::Other, format!("failed to lookup address information: {}", reason)))
        }
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

pub struct TcpConnection {
    // TODO: should the VecDeque be bounded?
    buffers: VecDeque<Buffer>,
    stream: TcpStream,
}

impl TcpConnection {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            buffers: VecDeque::new(),
            stream,
        }
    }

    fn as_raw_fd(&self) -> RawFd {
        self.stream.as_raw_fd()
    }

    pub fn ip4<C>(process_queue: &ProcessQueue, event_loop: &EventLoop, host: &str, port: u16, connection: C)
    where C: TcpConnectionNotify + Send + 'static,
    {
        tcp::connect_to_host(host, &port.to_string(), process_queue, event_loop, connection);
    }

    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buffer)
    }

    pub fn write(&mut self, buffer: Vec<u8>) -> io::Result<()> {
        let buffer_size = buffer.len();
        let mut stream = self.stream.try_clone()?;
        let mut index = 0;
        loop {
            match stream.write(&buffer[index..]) {
                Err(ref error) if error.kind() == ErrorKind::WouldBlock => {
                    self.buffers.push_back(Buffer::new(buffer, index));
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
    }
}

pub trait TcpListenNotify {
    fn listening(&mut self, _listener: &net::TcpListener) {
    }

    fn not_listening(&mut self) {
    }

    fn closed(&mut self, _listener: &net::TcpListener) {
    }

    fn connected(&mut self, listener: &net::TcpListener) -> Box<TcpConnectionNotify + Send>; // TODO: maybe remove Send.

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

pub enum Msg {
}

pub struct TcpListener {
}

fn manage_connection(eloop: &EventLoop, mut connection: TcpConnection, mut connection_notify: Box<TcpConnectionNotify>) {
    connection_notify.connected(&mut connection); // TODO: is this second method necessary?
    let fd = connection.as_raw_fd();
    let event_loop = eloop.clone();
    let result = eloop.try_add_raw_fd(fd, Mode::ReadWrite);
    match result {
        Ok(mut event) =>
            event.set_callback(move |event| {
                if (event.events & Mode::HangupError as u32) != 0 ||
                    (event.events & Mode::ShutDown as u32) != 0
                {
                    if let Err(error) = event_loop.remove_raw_fd(fd) {
                        // TODO: not sure if it makes sense to report this error to the user.
                        connection_notify.error(error);
                    }
                    return;
                }
                if event.events & Mode::Read as u32 != 0 {
                    loop {
                        // Loop to read everything because the edge-triggered mode is
                        // used and it only notifies once per readiness.
                        // TODO: Might want to reschedule the read to avoid starvation
                        // of other sockets.
                        let mut buffer = vec![0; 4096];
                        match connection.read(&mut buffer) {
                            Err(ref error) if error.kind() == ErrorKind::WouldBlock ||
                                error.kind() == ErrorKind::Interrupted => break,
                            Ok(bytes_read) => {
                                if bytes_read == 0 {
                                    // The connection has been shut down.
                                    break;
                                }
                                buffer.truncate(bytes_read);
                                connection_notify.received(&mut connection, buffer);
                            },
                            _ => (),
                        }
                    }
                }
                if event.events & Mode::Write as u32 != 0 {
                    let mut remove_buffer = false;
                    // TODO: yield sometimes to avoid starvation?
                    loop {
                        if let Some(ref mut first_buffer) = connection.buffers.front_mut() {
                            match connection.stream.write(first_buffer.slice()) {
                                Ok(written) => {
                                    first_buffer.advance(written);
                                    if first_buffer.exhausted() {
                                        remove_buffer = true;
                                    }
                                },
                                Err(ref error) if error.kind() == ErrorKind::WouldBlock => break,
                                Err(ref error) if error.kind() == ErrorKind::Interrupted => (),
                                Err(error) => connection_notify.error(error),
                            }
                        }
                        else {
                            break;
                        }
                        if remove_buffer {
                            connection.buffers.pop_front();
                        }
                    }
                }
            }),
        Err(error) => connection_notify.error(error),
    }
}

impl TcpListener {
    pub fn ip4<L>(event_loop: &EventLoop, host: &str, mut listen_notify: L)
        -> io::Result<impl FnMut(&Pid<Msg>, Option<Msg>) -> ProcessContinuation>
    where L: TcpListenNotify + Send + 'static,
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
        let eloop = event_loop.clone();
        event_loop.add_raw_fd(tcp_listener.as_raw_fd(), Mode::Read, move |event| {
            // TODO: check errors in event.
            if event.events & Mode::Read as u32 != 0 {
                match tcp_listener.accept() {
                    Ok((stream, _addr)) => {
                        match stream.set_nonblocking(true) {
                            Ok(()) => {
                                let mut connection_notify = listen_notify.connected(&tcp_listener);
                                let mut connection = TcpConnection::new(stream);
                                connection_notify.accepted(&mut connection);
                                manage_connection(&eloop, connection, connection_notify);
                            },
                            Err(error) => listen_notify.error(error),
                        }
                    },
                    Err(ref error) if error.kind() == ErrorKind::WouldBlock => {
                    },
                    Err(error) => listen_notify.error(error),
                }
            }
        })?;
        // TODO: call listen_notify.closed().
        Ok(|_current: &Pid<_>, _msg| {
            // TODO: have a message Dispose to stop listening.
            ProcessContinuation::WaitMessage
        })
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
