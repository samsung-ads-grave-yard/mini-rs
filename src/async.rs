use std::collections::VecDeque;
use std::io;
use std::io::{
    Error,
    ErrorKind,
    Read,
    Write,
};
use std::net;
use std::net::TcpStream;
use std::os::unix::io::{AsRawFd, RawFd};
use std::ptr;

use actor::{
    Pid,
    ProcessQueue,
    ProcessContinuation,
};
use net::tcp::connect_to_host;

const MAX_EVENTS: usize = 100;

#[repr(u32)]
pub enum Mode {
    HangupError = ffi::EPOLLHUP,
    Read = ffi::EPOLLIN | ffi::EPOLLET | ffi::EPOLLRDHUP,
    ReadWrite = ffi::EPOLLIN | ffi::EPOLLOUT | ffi::EPOLLET | ffi::EPOLLRDHUP,
    ShutDown = ffi::EPOLLRDHUP,
    Write = ffi::EPOLLOUT | ffi::EPOLLET | ffi::EPOLLRDHUP,
}

pub enum EpollResult {
    Error(io::Error),
    Interrupted,
    Ok,
}

#[derive(Clone)]
pub struct EventLoop {
    fd: RawFd,
}

impl EventLoop {
    pub fn new() -> io::Result<Self> {
        let fd = unsafe { ffi::epoll_create1(0) };
        if fd == -1 {
            return Err(Error::last_os_error());
        }
        Ok(Self {
            fd,
        })
    }

    fn add_fd<F, S>(&self, socket: &S, mode: Mode, callback: F) -> io::Result<()>
    where F: FnMut(ffi::epoll_event) + 'static,
          S: AsRawFd,
    {
        self.add_raw_fd(socket.as_raw_fd(), mode, callback)
    }

    pub fn add_raw_fd<F>(&self, fd: RawFd, mode: Mode, callback: F) -> io::Result<()>
    where F: FnMut(ffi::epoll_event) + 'static,
    {
        let callback: Box<Box<FnMut(ffi::epoll_event) + 'static>> = Box::new(Box::new(callback));
        let callback_pointer = Box::into_raw(callback);
        // TODO: give the reponsibility to the caller to destroy the callback. Send a message when
        // the event is a hangup to allow the caller to destroy the callback.
        let mut event = ffi::epoll_event {
            events: mode as u32,
            data: ffi::epoll_data_t {
                u64: callback_pointer as u64,
            },
        };
        if unsafe { ffi::epoll_ctl(self.fd, ffi::EpollOperation::Add, fd, &mut event) } == -1 {
            return Err(Error::last_os_error());
        }
        Ok(())
    }

    pub fn add_raw_fd_oneshot<F>(&self, fd: RawFd, mode: Mode, callback: F) -> io::Result<()>
    where F: FnOnce(ffi::epoll_event) + 'static,
    {
        let callback: Box<Box<FnOnce(ffi::epoll_event) + 'static>> = Box::new(Box::new(callback));
        let callback_pointer = Box::into_raw(callback);
        // TODO: give the reponsibility to the caller to destroy the callback. Send a message when
        // the event is a hangup to allow the caller to destroy the callback.
        let mut event = ffi::epoll_event {
            events: mode as u32 | ffi::EPOLLONESHOT,
            data: ffi::epoll_data_t {
                u64: callback_pointer as u64,
            },
        };
        if unsafe { ffi::epoll_ctl(self.fd, ffi::EpollOperation::Add, fd, &mut event) } == -1 {
            return Err(Error::last_os_error());
        }
        Ok(())
    }

    pub fn remove_raw_fd(&self, fd: RawFd) -> io::Result<()> {
        if unsafe { ffi::epoll_ctl(self.fd, ffi::EpollOperation::Delete, fd, ptr::null_mut()) } == -1 {
            return Err(Error::last_os_error());
        }
        Ok(())
    }

    pub fn iterate(&self, event_list: &mut [ffi::epoll_event]) -> EpollResult {
        // NOTE: Do not use self.callbacks, only use self.fd.
        // This is because a callback could call add_fd() which would cause a BorrowMut error.
        // We instead get the callback from the epoll data.
        let epoll_fd = self.fd;

        // TODO: check if epoll_wait() can be called from multiple threads.
        let ready = unsafe { ffi::epoll_wait(epoll_fd, event_list.as_mut_ptr(), event_list.len() as i32, -1) };
        if ready == -1 {
            let last_error = Error::last_os_error();
            if last_error.kind() == ErrorKind::Interrupted {
                return EpollResult::Interrupted;
            }
            else {
                return EpollResult::Error(last_error);
            }
        }

        for i in 0..ready as usize {
            let event = event_list[i];
            // Safety: it's safe to access the callback as a mutable reference here because the other accesses
            // to callbacks will add element to the slab, not access a random element
            // concurrently.
            let callback =  unsafe { &mut *(event.data.u64 as *mut Box<FnMut(ffi::epoll_event)>) };
            callback(event);
        }

        EpollResult::Ok
    }

    pub fn run(&self) -> io::Result<()> {
        let mut event_list = event_list();

        loop {
            match self.iterate(&mut event_list) {
                // Restart if interrupted by signal.
                EpollResult::Interrupted => continue,
                EpollResult::Error(error) => return Err(error),
                EpollResult::Ok => (),
            }
        }
    }
}

pub fn event_list() -> [ffi::epoll_event; MAX_EVENTS] {
    [
        ffi::epoll_event {
            events: 0,
            data: ffi::epoll_data_t {
                u32: 0,
            }
        }; MAX_EVENTS
    ]
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
    where C: TcpConnectionNotify+ Send + 'static,
    {
        connect_to_host(host, &port.to_string(), process_queue, event_loop, connection);
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
}

pub enum Msg {
}

pub struct TcpListener {
}

pub fn manage_connection(eloop: &EventLoop, mut connection: TcpConnection, mut connection_notify: Box<TcpConnectionNotify>) {
    connection_notify.connected(&mut connection); // TODO: is this second method necessary?
    let fd = connection.as_raw_fd();
    let event_loop = eloop.clone();
    eloop.add_raw_fd(fd, Mode::ReadWrite, move |event| {
        if (event.events & Mode::HangupError as u32) != 0 ||
            (event.events & Mode::ShutDown as u32) != 0
        {
            event_loop.remove_raw_fd(fd);
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
                        Err(ref error) => {
                            // TODO: handle errors.
                            panic!("IO error: {}", error);
                        },
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
    }).expect("add_raw_fd");
}

impl TcpListener {
    pub fn ip4<L>(event_loop: &EventLoop, host: &str, mut listener: L)
        -> io::Result<impl FnMut(&Pid<Msg>, Option<Msg>) -> ProcessContinuation>
    where L: TcpListenNotify + Send + 'static,
    {
        let tcp_listener =
            match net::TcpListener::bind(host) {
                Ok(tcp_listener) => {
                    listener.listening(&tcp_listener);
                    tcp_listener
                },
                Err(error) => {
                    listener.not_listening();
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
                        stream.set_nonblocking(true); // TODO: handle error.
                        let mut connection_notify = listener.connected(&tcp_listener);
                        let mut connection = TcpConnection::new(stream);
                        connection_notify.accepted(&mut connection);
                        manage_connection(&eloop, connection, connection_notify);
                    },
                    Err(ref error) if error.kind() == ErrorKind::WouldBlock => {
                    },
                    Err(ref error) => {
                        // TODO: handle errors.
                        panic!("IO error: {}", error);
                    },
                }
            }
        })?;
        // TODO: call listener.closed().
        Ok(|_current: &Pid<_>, _msg| {
            // TODO: have a message Dispose to stop listening.
            ProcessContinuation::WaitMessage
        })
    }
}

mod ffi {
    use std::os::raw::c_void;

    #[repr(i32)]
    pub enum EpollOperation {
        Add = 1,
        Delete = 2,
        Modify = 3,
    }

    pub const EPOLLIN: u32 = 0x001;
    pub const EPOLLOUT: u32 = 0x004;
    pub const EPOLLERR: u32 = 0x008;
    pub const EPOLLONESHOT: u32 = 1 << 30;
    pub const EPOLLET: u32 = 1 << 31;
    pub const EPOLLHUP: u32 = 0x010;
    pub const EPOLLRDHUP: u32 = 0x2000;

   #[repr(C)]
    #[derive(Clone, Copy)]
    pub union epoll_data_t {
        pub ptr: *mut c_void,
        pub fd: i32,
        pub u32: u32,
        pub u64: u64,
    }

    #[repr(C, packed)]
    #[derive(Clone, Copy)]
    pub struct epoll_event {
        pub events: u32,
        pub data: epoll_data_t,
    }

    extern "C" {
        pub fn epoll_create1(flags: i32) -> i32;
        pub fn epoll_ctl(epfd: i32, op: EpollOperation, fd: i32, event: *mut epoll_event) -> i32;
        pub fn epoll_wait(epdf: i32, events: *mut epoll_event, maxevents: i32, timeout: i32) -> i32;
    }
}
