/*
 * FIXME: that looks wrong to have so much allocations:
 * total heap usage: 1,824,371 allocs, 1,824,213 frees, 14,624,698 bytes allocated
 */

use std::io;
use std::io::{
    Error,
    ErrorKind,
};
use std::mem;
use std::os::unix::io::RawFd;
use std::ptr;

const MAX_EVENTS: usize = 100;

#[repr(u32)]
pub enum Mode {
    Error = ffi::EPOLLERR,
    HangupError = ffi::EPOLLHUP,
    Read = ffi::EPOLLIN | ffi::EPOLLET | ffi::EPOLLRDHUP,
    ReadWrite = ffi::EPOLLIN | ffi::EPOLLOUT | ffi::EPOLLET | ffi::EPOLLRDHUP,
    ShutDown = ffi::EPOLLRDHUP,
    Write = ffi::EPOLLOUT | ffi::EPOLLET | ffi::EPOLLRDHUP,
}

trait Callback {
    fn call(&mut self, event: ffi::epoll_event) -> Action;
}

struct EmptyCallback {
}

impl EmptyCallback {
    fn new() -> Self {
        Self {
        }
    }
}

impl Callback for EmptyCallback {
    fn call(&mut self, _event: ffi::epoll_event) -> Action {
        panic!("Found empty callback, did you forget to call Event[Once]::set_callback()?");
    }
}

struct NormalCallback<F> {
    callback: F,
}

impl<F> NormalCallback<F> {
    fn new(callback: F) -> Self {
        Self {
            callback,
        }
    }
}

impl<F> Callback for NormalCallback<F>
where F: FnMut(ffi::epoll_event) -> Action,
{
    fn call(&mut self, event: ffi::epoll_event) -> Action {
        (self.callback)(event)
    }
}

struct OneshotCallback<F> {
    callback: Option<F>,
}

impl<F> OneshotCallback<F> {
    fn new(callback: F) -> Self {
        Self {
            callback: Some(callback),
        }
    }
}

impl<F> Callback for OneshotCallback<F>
where F: FnOnce(ffi::epoll_event),
{
    fn call(&mut self, event: ffi::epoll_event) -> Action {
        let callback = mem::replace(&mut self.callback, None);
        if let Some(callback) = callback {
            callback(event);
        }
        Action::Stop
    }
}

pub enum Action {
    Continue,
    Stop,
}

pub struct Event {
    callback: *mut Box<Callback>,
}

impl Event {
    fn new(callback: Box<Box<Callback>>) -> Self {
        Self {
            callback: Box::into_raw(callback),
        }
    }

    pub fn set_callback<F>(&mut self, callback: F)
    where F: FnMut(ffi::epoll_event) -> Action + 'static,
    {
        unsafe {
            (*self.callback) = Box::new(NormalCallback::new(callback));
        }
    }
}

pub struct EventOnce {
    callback: *mut Box<Callback>,
}

impl EventOnce {
    fn new(callback: Box<Box<Callback>>) -> Self {
        Self {
            callback: Box::into_raw(callback),
        }
    }

    pub fn set_callback<F>(&mut self, callback: F)
    where F: FnOnce(ffi::epoll_event) + 'static,
    {
        unsafe {
            (*self.callback) = Box::new(OneshotCallback::new(callback));
        }
    }
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

    pub fn add_raw_fd<F>(&self, fd: RawFd, mode: Mode, callback: F) -> io::Result<()>
    where F: FnMut(ffi::epoll_event) -> Action + 'static,
    {
        let callback = Box::new(Box::new(NormalCallback::new(callback)));
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
        let callback = Box::new(Box::new(OneshotCallback::new(callback)));
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

    pub fn try_add_raw_fd(&self, fd: RawFd, mode: Mode) -> io::Result<Event> {
        let mut callback: Box<Box<Callback>> = Box::new(Box::new(EmptyCallback::new()));
        let callback_pointer = &mut *callback as *mut _;
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
        Ok(Event::new(callback))
    }

    pub fn try_add_raw_fd_oneshot(&self, fd: RawFd, mode: Mode) -> io::Result<EventOnce> {
        let mut callback: Box<Box<Callback>> = Box::new(Box::new(EmptyCallback::new()));
        let callback_pointer = &mut * callback as *mut _;
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
        Ok(EventOnce::new(callback))
    }

    pub fn iterate(&self, event_list: &mut [ffi::epoll_event]) -> EpollResult {
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

        for &event in event_list.iter().take(ready as usize) {
            // Safety: it's safe to access the callback as a mutable reference here because only
            // this function can access the callbacks since they are only stored in the epoll data.
            unsafe {
                let callback_pointer = event.data.u64 as *mut Box<Callback>;
                if let Action::Stop = (*callback_pointer).call(event) {
                    // Destroy the callback.
                    Box::from_raw(callback_pointer);
                }
            }
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

pub mod ffi {
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
