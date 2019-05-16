use std::io::{
    self,
    ErrorKind,
    Read,
    Stdin as StdStdin,
    stdin,
};

use aio::async::Mode;
use aio::async::ffi::epoll_event;
use aio::handler::{
    Loop,
    Handler,
    Stream,
};
use aio::net::set_nonblocking;

use self::Msg::*;

enum Msg {
    Read(epoll_event),
}

struct StdinHandler<NOTIFY> {
    input_notify: NOTIFY,
    stdin: StdStdin,
}

impl<NOTIFY> StdinHandler<NOTIFY> {
    fn new(input_notify: NOTIFY) -> io::Result<Self> {
        let stdin = stdin();
        set_nonblocking(&stdin)?;
        Ok(Self {
            input_notify,
            stdin,
        })
    }
}

impl<NOTIFY> Handler for StdinHandler<NOTIFY>
where NOTIFY: InputNotify,
{
    type Msg = Msg;

    fn update(&mut self, _stream: &Stream<Msg>, msg: Msg) {
        match msg {
            Read(event) => {
                if event.events & Mode::Read as u32 != 0 {
                    let mut buffer = vec![0; 4096];
                    match self.stdin.read(&mut buffer) {
                        Err(ref error) if error.kind() == ErrorKind::WouldBlock ||
                            error.kind() == ErrorKind::Interrupted => (),
                        Ok(bytes_read) => {
                            if bytes_read > 0 {
                                buffer.truncate(bytes_read);
                                self.input_notify.received(buffer);
                            }
                        },
                        _ => (),
                    }
                }
            },
        }
    }
}

pub struct Stdin {
}

impl Stdin {
    pub fn new<NOTIFY>(event_loop: &mut Loop, input_notify: NOTIFY) -> io::Result<()>
    where NOTIFY: InputNotify + 'static,
    {
        let stdin = stdin();
        set_nonblocking(&stdin)?;
        let stream = event_loop.spawn(StdinHandler::new(input_notify)?);
        event_loop.add_fd(&stdin, Mode::Read, &stream, Read)
    }
}

pub trait InputNotify {
    fn received(&mut self, data: Vec<u8>);
}
