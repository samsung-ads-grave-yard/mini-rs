use std::cell::RefCell;
use std::io;
use std::io::{Error, ErrorKind};
use std::net;
use std::net::TcpStream;
use std::os::unix::io::{AsRawFd, RawFd};
use std::rc::Rc;

use actor::{
    Pid,
    ProcessContinuation,
};
use collections::Slab;

const MAX_EVENTS: usize = 100;

#[repr(u32)]
pub enum Mode {
    Read = ffi::EPOLLIN,
    Write = ffi::EPOLLOUT,
}

#[derive(Clone)]
pub struct EventLoop {
    callbacks: Rc<RefCell<Slab<Box<FnMut(ffi::epoll_event)>>>>,
    fd: RawFd,
}

impl EventLoop {
    pub fn new() -> io::Result<Self> {
        let fd = unsafe { ffi::epoll_create1(0) };
        if fd == -1 {
            return Err(Error::last_os_error());
        }
        Ok(Self {
            callbacks: Rc::new(RefCell::new(Slab::new())),
            fd,
        })
    }

    pub fn add_fd<F, S>(&self, socket: &S, mode: Mode, callback: F) -> io::Result<()>
    where F: FnMut(ffi::epoll_event) + 'static,
          S: AsRawFd,
    {
        self.add_raw_fd(socket.as_raw_fd(), mode, callback)
    }

    pub fn add_raw_fd<F>(&self, fd: RawFd, mode: Mode, callback: F) -> io::Result<()>
    where F: FnMut(ffi::epoll_event) + 'static
    {
        let entry = self.callbacks.borrow_mut().entry();
        self.callbacks.borrow_mut().insert(entry, Box::new(callback));
        // TODO: remove the message when the fd is removed.
        let mut event = ffi::epoll_event {
            events: mode as u32,
            data: ffi::epoll_data_t {
                u64: entry as u64,
            },
        };
        println!("Add fd: {} with entry: {}", fd, entry);
        if unsafe { ffi::epoll_ctl(self.fd, ffi::EpollOperation::Add, fd, &mut event) } == -1 {
            return Err(Error::last_os_error());
        }
        Ok(())
    }

    pub fn run(&self) -> io::Result<()> {
        let mut event_list = [
            ffi::epoll_event {
                events: 0,
                data: ffi::epoll_data_t {
                    u32: 0,
                }
            }; MAX_EVENTS
        ];

        loop {
            println!("Waiting");
            let ready = unsafe { ffi::epoll_wait(self.fd, event_list.as_mut_ptr(), MAX_EVENTS as i32, -1) };
            println!("Waited");
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
            println!("Iterating");
            for i in 0..ready as usize {
                let event = event_list[i];
                //println!("Events: {:?}", event.events);
                let entry = unsafe { event.data.u64 } as usize;
                if let Some(callback) = self.callbacks.borrow_mut().get_mut(entry) {
                    println!("Call callback");
                    callback(event);
                }
                else {
                    panic!("Cannot find callback.");
                }
                println!("Entry: {}", entry);
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

    fn connected(&mut self, listener: &net::TcpListener) -> Box<TcpConnectionNotify>;
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
    pub fn ip4<L: TcpListenNotify + 'static>(event_loop: &mut EventLoop, mut listener: L)
        -> io::Result<impl FnMut(&Pid<Msg>, Option<Msg>) -> ProcessContinuation>
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
        event_loop.add_raw_fd(tcp_listener.as_raw_fd(), Mode::Read, move |event| {
            // TODO: check errors in event.
            if event.events & Mode::Read as u32 != 0 {
                match tcp_listener.accept() {
                    Ok((stream, _addr)) => {
                        stream.set_nonblocking(true); // TODO: handle error.
                        let connection = listener.connected(&tcp_listener);
                    },
                    Err(ref error) if error.kind() == ErrorKind::WouldBlock => {
                    },
                    Err(ref error) => {
                        // TODO: handle errors.
                        panic!("IO error: {}", error);
                    },
                }
            }
        }).expect("add fd");
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

    //#[repr(C, packed)]
    #[repr(packed)]
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
