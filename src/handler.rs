use std::cell::RefCell;
use std::collections::VecDeque;
use std::io;
use std::os::unix::io::{
    AsRawFd,
    RawFd,
};
use std::rc::Rc;

use async::{
    self,
    Action,
    EpollResult,
    EventLoop,
    Mode,
    event_list,
};
use async::ffi::epoll_event;
use slab::{Entry, Slab};

pub struct Stream<MSG> {
    elements: Rc<RefCell<VecDeque<MSG>>>,
}

impl<MSG> Clone for Stream<MSG> {
    fn clone(&self) -> Self {
        Self {
            elements: self.elements.clone(),
        }
    }
}

impl<MSG> Stream<MSG> {
    fn new() -> Self {
        Self {
            elements: Rc::new(RefCell::new(VecDeque::new())),
        }
    }

    fn pop(&self) -> Option<MSG> {
        self.elements.borrow_mut().pop_front()
    }

    pub fn send(&self, msg: MSG) {
        self.elements.borrow_mut().push_back(msg);
        EventLoop::wakeup();
    }
}

pub trait Handler {
    type Msg;

    fn update(&mut self, stream: &Stream<Self::Msg>, msg: Self::Msg);
}

struct Component<HANDLER: Handler<Msg=MSG>, MSG> {
    handler: HANDLER,
    stream: Stream<MSG>,
}

trait Callable {
    fn process(&mut self);
}

impl<HANDLER: Handler<Msg=MSG>, MSG> Callable for Component<HANDLER, MSG> {
    fn process(&mut self) {
        while let Some(msg) = self.stream.pop() {
            self.handler.update(&self.stream, msg);
        }
    }
}

#[derive(Clone)]
pub struct Loop {
    event_loop: EventLoop,
    handlers: Rc<RefCell<Slab<Box<Callable>>>>,
    stopped: bool,
}

impl Loop {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            event_loop: EventLoop::new()?,
            handlers: Rc::new(RefCell::new(Slab::new())),
            stopped: false,
        })
    }

    pub fn add_fd<A: AsRawFd, CALLBACK, MSG>(&self, as_fd: &A, mode: Mode, stream: &Stream<MSG>, callback: CALLBACK) -> io::Result<()>
    where CALLBACK: Fn(epoll_event) -> MSG + 'static,
          MSG: 'static,
    {
        self.add_raw_fd(as_fd.as_raw_fd(), mode, stream, callback)
    }

    pub fn add_raw_fd<CALLBACK, MSG>(&self, fd: RawFd, mode: Mode, stream: &Stream<MSG>, callback: CALLBACK) -> io::Result<()>
    where CALLBACK: Fn(epoll_event) -> MSG + 'static,
          MSG: 'static,
    {
        let stream = stream.clone();
        self.event_loop.add_raw_fd(fd, mode, move |event| {
            stream.send(callback(event));
            Action::Continue
        })
    }

    pub fn event_loop(&self) -> &EventLoop {
        &self.event_loop
    }

    pub fn spawn<HANDLER, MSG>(&mut self, handler: HANDLER) -> Stream<MSG>
    where HANDLER: Handler<Msg=MSG> + 'static,
          MSG: 'static,
    {
        let stream = Stream::new();
        self.handlers.borrow_mut().insert(Box::new(Component {
            handler,
            stream: stream.clone(),
        }));
        // TODO: think about how to remove the components.
        stream
    }

    pub fn iterate(&mut self, event_list: &mut [epoll_event]) -> EpollResult {
        let capacity = self.handlers.borrow().capacity();
        for index in 0..capacity {
            let entry = Entry::from(index);
            let value = self.handlers.borrow_mut().reserve_remove(entry);
            if let Some(mut handler) = value {
                handler.process();
                self.handlers.borrow_mut().set(entry, handler);
            }
        }
        self.event_loop.iterate(event_list)
    }

    pub fn remove_fd<A: AsRawFd>(&self, as_fd: &A) -> io::Result<()> {
        self.event_loop.remove_fd(as_fd)
    }

    pub fn remove_raw_fd(&self, fd: RawFd) -> io::Result<()> {
        self.event_loop.remove_raw_fd(fd)
    }

    pub fn run(&mut self) -> io::Result<()> {
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

    pub fn try_add_fd<A: AsRawFd>(&self, as_fd: &A, mode: Mode) -> io::Result<Event> {
        self.try_add_raw_fd(as_fd.as_raw_fd(), mode)
    }

    pub fn try_add_raw_fd(&self, fd: RawFd, mode: Mode) -> io::Result<Event> {
        Ok(Event::new(self.event_loop.try_add_raw_fd(fd, mode)?))
    }

    pub fn try_add_raw_fd_oneshot(&self, fd: RawFd, mode: Mode) -> io::Result<EventOnce> {
        Ok(EventOnce::new(self.event_loop.try_add_raw_fd_oneshot(fd, mode)?))
    }
}

pub struct Event {
    event: async::Event,
}

impl Event {
    fn new(event: async::Event) -> Self {
        Self {
            event,
        }
    }

    pub fn set_callback<CALLBACK, MSG>(self, stream: &Stream<MSG>, callback: CALLBACK)
    where CALLBACK: Fn(epoll_event) -> MSG + 'static,
          MSG: 'static,
    {
        let stream = stream.clone();
        self.event.set_callback(move |event| {
            stream.send(callback(event));
            Action::Continue
        });
    }
}

pub struct EventOnce {
    event: async::EventOnce,
}

impl EventOnce {
    fn new(event: async::EventOnce) -> Self {
        Self {
            event,
        }
    }

    pub fn set_callback<CALLBACK, MSG>(self, stream: &Stream<MSG>, callback: CALLBACK)
    where CALLBACK: FnOnce(epoll_event) -> MSG + 'static,
          MSG: 'static,
    {
        let stream = stream.clone();
        self.event.set_callback(move |event| stream.send(callback(event)));
    }
}
