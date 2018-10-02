use std::cell::RefCell;
use std::io;
use std::io::{Error, ErrorKind, Read};
use std::net;
use std::net::TcpStream;
use std::os::unix::io::{AsRawFd, RawFd};
use std::ptr;
use std::rc::Rc;

use actor::{
    Pid,
    ProcessContinuation,
};
use collections::Slab;

const MAX_EVENTS: usize = 100;

#[repr(u32)]
pub enum Mode {
    HangupError = ffi::EPOLLHUP | ffi::EPOLLERR,
    Read = ffi::EPOLLIN,
    ReadWrite = ffi::EPOLLIN | ffi::EPOLLOUT,
    Write = ffi::EPOLLOUT,
}

#[derive(Clone)]
pub struct EventLoop {
    callbacks: Rc<RefCell<Slab<Box<Box<FnMut(ffi::epoll_event)>>>>>,
    fd: RawFd,
}

impl EventLoop {
    pub fn new() -> io::Result<Self> {
        // TODO: probably wants to switch to edge triggered.
        let fd = unsafe { ffi::epoll_create1(0) };
        if fd == -1 {
            return Err(Error::last_os_error());
        }
        Ok(Self {
            callbacks: Rc::new(RefCell::new(Slab::new())),
            fd,
        })
    }

    fn add_fd<F, S>(&self, socket: &S, mode: Mode, callback: F) -> io::Result<()>
    where F: FnMut(ffi::epoll_event) + 'static,
          S: AsRawFd,
    {
        self.add_raw_fd(socket.as_raw_fd(), mode, callback)
    }

    fn add_raw_fd<F>(&self, fd: RawFd, mode: Mode, callback: F) -> io::Result<()>
    where F: FnMut(ffi::epoll_event) + 'static,
    {
        let mut callbacks = self.callbacks.borrow_mut();
        let entry = callbacks.entry();
        let callback: Box<Box<FnMut(ffi::epoll_event) + 'static>> = Box::new(Box::new(callback));
        let callback_pointer = &*callback as *const _;
        callbacks.insert(entry, callback);
        // TODO: remove the message when the fd is removed.
        let mut event = ffi::epoll_event {
            events: mode as u32,
            data: ffi::epoll_data_t {
                u64: callback_pointer as u64,
            },
        };
        println!("Add fd: {} with entry: {}", fd, entry);
        if unsafe { ffi::epoll_ctl(self.fd, ffi::EpollOperation::Add, fd, &mut event) } == -1 {
            return Err(Error::last_os_error());
        }
        Ok(())
    }

    fn remove_raw_fd(&self, fd: RawFd) -> io::Result<()> {
        if unsafe { ffi::epoll_ctl(self.fd, ffi::EpollOperation::Delete, fd, ptr::null_mut()) } == -1 {
            return Err(Error::last_os_error());
        }
        Ok(())
    }

    pub fn run(&self) -> io::Result<()> {
        // NOTE: Do not use self.callbacks, only use self.fd.
        // This is because a callback could call add_fd() which would cause a BorrowMut error.
        // We instead get the callback from the epoll data.
        let epoll_fd = self.fd;

        let mut event_list = [
            ffi::epoll_event {
                events: 0,
                data: ffi::epoll_data_t {
                    u32: 0,
                }
            }; MAX_EVENTS
        ];

        loop {
            let ready = unsafe { ffi::epoll_wait(epoll_fd, event_list.as_mut_ptr(), MAX_EVENTS as i32, -1) };
            if ready == -1 {
                let last_error = Error::last_os_error();
                if last_error.kind() == ErrorKind::Interrupted {
                    // Restart if interrupted by signal.
                    continue;
                }
                else {
                    return Err(last_error);
                }
            }

            for i in 0..ready as usize {
                let event = event_list[i];
                let callback =  unsafe { &mut *(event.data.u64 as *mut Box<FnMut(ffi::epoll_event)>) };
                callback(event);
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
    fn accepted(&mut self, _connection: &mut TcpStream) {
    }

    fn connecting(&mut self, _connection: &mut TcpStream, _count: u32) {
    }

    fn connected(&mut self, _connection: &mut TcpStream) {
    }

    fn connect_failed(&mut self, _connection: &mut TcpStream) {
    }

    fn auth_failed(&mut self, _connection: &mut TcpStream) {
    }

    fn sent(&mut self, _connection: &mut TcpStream, data: Vec<u8>) -> Vec<u8> {
        data
    }

    fn received(&mut self, _connection: &mut TcpStream, _data: Vec<u8>) {
    }

    fn closed(&mut self, _connection: &mut TcpStream) {
    }
}

pub enum Msg {
}

pub struct TcpListener {
}

impl TcpListener {
    pub fn ip4<L>(event_loop: &EventLoop, mut listener: L)
        -> io::Result<impl FnMut(&Pid<Msg>, Option<Msg>) -> ProcessContinuation>
    where L: TcpListenNotify + Send + 'static,
    {
        let tcp_listener =
            match net::TcpListener::bind("127.0.0.1:1337") { // TODO: allow to specify the port.
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
                    Ok((mut stream, _addr)) => {
                        stream.set_nonblocking(true); // TODO: handle error.
                        let mut connection = listener.connected(&tcp_listener);
                        connection.accepted(&mut stream);
                        connection.connected(&mut stream); // TODO: is this second method necessary?
                        let stream_fd = stream.as_raw_fd();
                        let event_loop = eloop.clone();
                        eloop.add_raw_fd(stream_fd, Mode::ReadWrite, move |event| {
                            if (event.events & Mode::HangupError as u32) != 0 {
                                event_loop.remove_raw_fd(stream_fd);
                                return;
                            }
                            if event.events & Mode::Read as u32 != 0 {
                                let mut buffer = vec![0; 4096];
                                // TODO: maybe read more than once?
                                stream.read(&mut buffer);
                                connection.received(&mut stream, buffer);
                                //println!("Read: {}", String::from_utf8_lossy(&buffer));
                            }
                            if event.events & Mode::Write as u32 != 0 {
                                //println!("Write");
                            }
                        }
                        );
                    },
                    Err(ref error) if error.kind() == ErrorKind::WouldBlock => {
                    },
                    Err(ref error) => {
                        // TODO: handle errors.
                        panic!("IO error: {}", error);
                    },
                }
            }
        });
        // TODO: call listener.closed().
        Ok(|current: &Pid<_>, msg| {
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
    pub const EPOLLHUP: u32 = 0x010;

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
