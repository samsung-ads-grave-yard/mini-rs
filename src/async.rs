/*
 * FIXME: that looks wrong to have so much allocations:
 * total heap usage: 1,824,371 allocs, 1,824,213 frees, 14,624,698 bytes allocated
 */

use std::cell::RefCell;
use std::io;
use std::io::{
    Error,
    ErrorKind,
};
use std::os::unix::io::RawFd;
use std::ptr;
use std::rc::Rc;
use std::u64;

use slab::{Entry, Slab};

const MAX_EVENTS: usize = 100; // TODO: tweak this value.

#[repr(u32)]
pub enum Mode {
    Error = ffi::EPOLLERR,
    HangupError = ffi::EPOLLHUP,
    Read = ffi::EPOLLIN | ffi::EPOLLRDHUP,
    ReadWrite = ffi::EPOLLIN | ffi::EPOLLOUT | ffi::EPOLLRDHUP,
    ShutDown = ffi::EPOLLRDHUP,
    Write = ffi::EPOLLOUT | ffi::EPOLLRDHUP,
}

trait FnBox {
    fn call_box(self: Box<Self>, event: ffi::epoll_event);
}

impl<T> FnBox for T where T: FnOnce(ffi::epoll_event) {
    fn call_box(self: Box<Self>, event: ffi::epoll_event) {
        (*self)(event);
    }
}

enum Callback {
    Normal(Box<FnMut(ffi::epoll_event) -> Action>),
    Oneshot(Box<FnBox>),
}

#[derive(PartialEq)]
pub enum Action {
    Continue,
    Stop,
}

pub struct Event {
    callback_entry: Entry,
    event_loop: EventLoop,
}

impl Event {
    fn new(callback_entry: Entry, event_loop: &EventLoop) -> Self {
        Self {
            callback_entry,
            event_loop: event_loop.clone(),
        }
    }

    pub fn set_callback<F>(self, callback: F)
    where F: FnMut(ffi::epoll_event) -> Action + 'static,
    {
        self.event_loop.callbacks.borrow_mut().set(self.callback_entry, Callback::Normal(Box::new(callback)));
    }
}

pub struct EventOnce {
    callback_entry: Entry,
    event_loop: EventLoop,
}

impl EventOnce {
    fn new(callback_entry: Entry, event_loop: EventLoop) -> Self {
        Self {
            callback_entry,
            event_loop: event_loop.clone(),
        }
    }

    pub fn set_callback<F>(self, callback: F)
    where F: FnOnce(ffi::epoll_event) + 'static,
    {
        self.event_loop.callbacks.borrow_mut().set(self.callback_entry, Callback::Oneshot(Box::new(callback)));
    }
}

pub enum EpollResult {
    Error(io::Error),
    Interrupted,
    Ok,
}

thread_local! {
    static EVENT_FD: RawFd = unsafe { ffi::eventfd(0, ffi::EFD_NONBLOCK) };
}

#[derive(Clone)]
pub struct EventLoop {
    callbacks: Rc<RefCell<Slab<Callback>>>,
    fd: RawFd,
    stopped: bool,
}

impl EventLoop {
    pub fn new() -> io::Result<Self> {
        // TODO: use EPOLL_EXCLUSIVE to allow using from multiple threads.
        let fd = unsafe { ffi::epoll_create1(0) };
        if fd == -1 {
            return Err(Error::last_os_error());
        }
        let event_loop = Self {
            callbacks: Rc::new(RefCell::new(Slab::new())),
            fd,
            stopped: false,
        };

        let event_fd = EVENT_FD.with(|&event_fd| event_fd);
        event_loop.add_raw_fd_without_callback(event_fd, Mode::Read)?;

        Ok(event_loop)
    }

    fn add_raw_fd_without_callback(&self, fd: RawFd, mode: Mode) -> io::Result<()> {
        let mut event = ffi::epoll_event {
            events: mode as u32,
            data: ffi::epoll_data_t {
                u64: u64::MAX,
            },
        };
        if unsafe { ffi::epoll_ctl(self.fd, ffi::EpollOperation::Add, fd, &mut event) } == -1 {
            return Err(Error::last_os_error());
        }
        Ok(())
    }

    pub fn add_raw_fd<F>(&self, fd: RawFd, mode: Mode, callback: F) -> io::Result<()>
    where F: FnMut(ffi::epoll_event) -> Action + 'static,
    {
        let callback_entry = self.callbacks.borrow_mut().insert(Callback::Normal(Box::new(callback)));
        let mut event = ffi::epoll_event {
            events: mode as u32,
            data: ffi::epoll_data_t {
                u64: callback_entry.index() as u64,
            },
        };
        if unsafe { ffi::epoll_ctl(self.fd, ffi::EpollOperation::Add, fd, &mut event) } == -1 {
            // TODO: should probably deallocate memory here.
            return Err(Error::last_os_error());
        }
        Ok(())
    }

    pub fn add_raw_fd_oneshot<F>(&self, fd: RawFd, mode: Mode, callback: F) -> io::Result<()>
    where F: FnOnce(ffi::epoll_event) + 'static,
    {
        let callback_entry = self.callbacks.borrow_mut().insert(Callback::Oneshot(Box::new(callback)));
        let mut event = ffi::epoll_event {
            events: mode as u32 | ffi::EPOLLONESHOT,
            data: ffi::epoll_data_t {
                u64: callback_entry.index() as u64,
            },
        };
        if unsafe { ffi::epoll_ctl(self.fd, ffi::EpollOperation::Add, fd, &mut event) } == -1 {
            // TODO: should probably deallocate memory here.
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
        let callback_entry = self.callbacks.borrow_mut().reserve_entry();
        let mut event = ffi::epoll_event {
            events: mode as u32,
            data: ffi::epoll_data_t {
                u64: callback_entry.index() as u64,
            },
        };
        if unsafe { ffi::epoll_ctl(self.fd, ffi::EpollOperation::Add, fd, &mut event) } == -1 {
            // TODO: should probably deallocate memory here.
            return Err(Error::last_os_error());
        }
        Ok(Event::new(callback_entry, self))
    }

    pub fn try_add_raw_fd_oneshot(&self, fd: RawFd, mode: Mode) -> io::Result<EventOnce> {
        let callback_entry = self.callbacks.borrow_mut().reserve_entry();
        let mut event = ffi::epoll_event {
            events: mode as u32 | ffi::EPOLLONESHOT,
            data: ffi::epoll_data_t {
                u64: callback_entry.index() as u64,
            },
        };
        if unsafe { ffi::epoll_ctl(self.fd, ffi::EpollOperation::Add, fd, &mut event) } == -1 {
            // TODO: should probably deallocate memory here.
            return Err(Error::last_os_error());
        }
        Ok(EventOnce::new(callback_entry, self.clone()))
    }

    pub fn iterate(&self, event_list: &mut [ffi::epoll_event]) -> EpollResult {
        let epoll_fd = self.fd;

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
            unsafe {
                if event.data.u64 == u64::MAX {
                    // No callback is associated with the eventfd used to wakeup the event loop.
                    EVENT_FD.with(|&event_fd| {
                        let mut value = 0u64;
                        ffi::eventfd_read(event_fd, &mut value as *mut _)
                    });
                    continue;
                }
            }
            let entry = unsafe { Entry::from(event.data.u64 as usize) };
            let callback =
                match self.callbacks.borrow_mut().remove(entry) {
                    Some(mut callback) => {
                        match callback {
                            Callback::Normal(mut callback) => {
                                if callback(event) == Action::Stop {
                                    None
                                }
                                else {
                                    Some(Callback::Normal(callback))
                                }
                            },
                            Callback::Oneshot(callback) => {
                                let callback: Box<_> = callback;
                                callback.call_box(event);
                                None
                            },
                        }
                    },
                    None => panic!("No callback"),
                };
            if let Some(callback) = callback {
                self.callbacks.borrow_mut().set(entry, callback);
            }
        }

        EpollResult::Ok
    }

    pub fn run(&self) -> io::Result<()> {
        let mut event_list = event_list();

        while !self.stopped {
            match self.iterate(&mut event_list) {
                // Restart if interrupted by signal.
                EpollResult::Interrupted => continue,
                EpollResult::Error(error) => return Err(error),
                EpollResult::Ok => (),
            }
        }

        Ok(())
    }

    pub fn stop(&mut self) {
        self.stopped = true;
        EventLoop::wakeup();
    }

    pub fn wakeup() {
        EVENT_FD.with(|&event_fd| {
            unsafe {
                ffi::eventfd_write(event_fd, 1);
            }
        });
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
    pub const EPOLLHUP: u32 = 0x010;
    pub const EPOLLRDHUP: u32 = 0x2000;
    pub const EFD_NONBLOCK: i32 = 0o4000;

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

    #[allow(non_camel_case_types)]
    type eventfd_t = u64;

    extern "C" {
        pub fn epoll_create1(flags: i32) -> i32;
        pub fn epoll_ctl(epfd: i32, op: EpollOperation, fd: i32, event: *mut epoll_event) -> i32;
        pub fn epoll_wait(epdf: i32, events: *mut epoll_event, maxevents: i32, timeout: i32) -> i32;

        pub fn eventfd(initval: u32, flags: i32) -> i32;
        pub fn eventfd_read(fd: i32, value: *mut eventfd_t) -> i32;
        pub fn eventfd_write(fd: i32, value: eventfd_t) -> i32;
    }
}
