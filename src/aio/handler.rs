use std::cell::RefCell;
use std::collections::VecDeque;
use std::io;
use std::mem;
use std::os::unix::io::{
    AsRawFd,
    RawFd,
};
use std::rc::Rc;

use crate::aio::poll::{
    self,
    Action,
    EpollResult,
    EventLoop,
    Mode,
    event_list,
};
use crate::aio::poll::ffi::epoll_event;
use crate::aio::slab::Slab;

pub struct Stream<MSG> {
    elements: Rc<RefCell<VecDeque<MSG>>>,
    entry: usize,
    registered_entries: Rc<RefCell<Vec<usize>>>,
}

impl<MSG> Clone for Stream<MSG> {
    fn clone(&self) -> Self {
        Self {
            elements: self.elements.clone(),
            entry: self.entry.clone(),
            registered_entries: self.registered_entries.clone(),
        }
    }
}

impl<MSG> Stream<MSG> {
    fn new(registered_entries: Rc<RefCell<Vec<usize>>>, entry: usize) -> Self {
        Self {
            elements: Rc::new(RefCell::new(VecDeque::new())),
            entry,
            registered_entries,
        }
    }

    fn pop(&self) -> Option<MSG> {
        self.elements.borrow_mut().pop_front()
    }

    pub fn send(&self, msg: MSG) {
        self.elements.borrow_mut().push_back(msg);
        self.registered_entries.borrow_mut().push(self.entry);
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

struct NotCallable;

impl Callable for NotCallable {
    fn process(&mut self) {
        panic!("Not callable");
    }
}

impl<HANDLER: Handler<Msg=MSG>, MSG> Callable for Component<HANDLER, MSG> {
    fn process(&mut self) {
        while let Some(msg) = self.stream.pop() {
            self.handler.update(&self.stream, msg);
        }
    }
}

struct Inner {
    handlers: Slab<Box<dyn Callable>>,
    registered_entries: Rc<RefCell<Vec<usize>>>,
    stopped: bool,
}

#[derive(Clone)]
pub struct Loop {
    event_loop: EventLoop,
    inner: Rc<RefCell<Inner>>,
}

impl Loop {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            event_loop: EventLoop::new()?,
            inner: Rc::new(RefCell::new(Inner {
                handlers: Slab::new(),
                registered_entries: Rc::new(RefCell::new(vec![])),
                stopped: false,
            })),
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
        let mut inner = self.inner.borrow_mut();
        let registered_entries = inner.registered_entries.clone();
        let handlers = &mut inner.handlers;
        let entry = handlers.vacant_entry();
        let stream = Stream::new(registered_entries, entry.key());
        entry.insert(Box::new(Component {
            handler,
            stream: stream.clone(),
        }));
        // TODO: think about how to remove the components.
        stream
    }

    pub fn iterate(&mut self, event_list: &mut [epoll_event]) -> EpollResult {
        let registered_entries = mem::replace(&mut *self.inner.borrow().registered_entries.borrow_mut(), vec![]);
        for entry in registered_entries {
            if self.inner.borrow().handlers.contains(entry) {
                // NOTE: Remove the handler because handlers can be added in the update() method.
                let mut handler = std::mem::replace(&mut self.inner.borrow_mut().handlers[entry], Box::new(NotCallable));
                handler.process();
                self.inner.borrow_mut().handlers[entry] = handler;
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

        while !self.inner.borrow().stopped {
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
        self.inner.borrow_mut().stopped = true;
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
    event: poll::Event,
}

impl Event {
    fn new(event: poll::Event) -> Self {
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
    event: poll::EventOnce,
}

impl EventOnce {
    fn new(event: poll::EventOnce) -> Self {
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
