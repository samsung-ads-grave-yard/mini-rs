// TODO: make a web crawler example.

use std::fmt::Debug;
use std::io;
use std::io::ErrorKind;
use std::mem;

use actor::{
    Pid,
    ProcessQueue,
};
use async::EventLoop;
use net::{
    TcpConnection,
    TcpConnectionNotify,
};

#[derive(Clone)]
struct Connection<HANDLER> {
    buffer: Vec<u8>,
    content_length: usize,
    http_handler: HANDLER,
    uri: String,
}

impl<HANDLER> Connection<HANDLER> {
    fn new(uri: &str, http_handler: HANDLER) -> Self {
        Self {
            buffer: vec![],
            content_length: 0,
            http_handler,
            uri: uri.to_string(),
        }
    }
}

impl<HANDLER> Drop for Connection<HANDLER> {
    fn drop(&mut self) {
        println!("Drop");
    }
}

fn parse_headers(buffer: &[u8]) -> Option<usize> {
    // TODO: parse other headers.
    let mut size = 0;
    for line in buffer.split(|byte| *byte == b'\n') {
        if line.starts_with(b"Content-Length:") {
            let mut parts = line.split(|byte| *byte == b':');
            parts.next()?;
            let mut value = String::from_utf8_lossy(parts.next()?);
            size = str::parse(value.trim()).ok()?;
        }
    }
    Some(size)
}

impl<HANDLER> TcpConnectionNotify for Connection<HANDLER>
where HANDLER: HttpHandler,
{
    fn connecting(&mut self, _connection: &mut TcpConnection, count: u32) {
        println!("Connecting. Attempt #{}", count);
    }

    fn connected(&mut self, connection: &mut TcpConnection) {
        if let Err(error) = connection.write(format!("GET / HTTP/1.1\r\nHost: {}\r\n\r\n", self.uri).into_bytes()) {
            self.http_handler.error(error);
        }
    }

    fn error(&mut self, error: io::Error) {
        self.http_handler.error(error);
    }

    fn received(&mut self, _connection: &mut TcpConnection, data: Vec<u8>) {
        self.buffer.extend(data);
        if self.buffer.ends_with(b"\r\n\r\n") {
            match parse_headers(&self.buffer) {
                Some(content_length) => {
                    self.content_length = content_length;
                    let mut buffer = vec![];
                    mem::swap(&mut self.buffer, &mut buffer);
                },
                None => self.http_handler.error(ErrorKind::InvalidData.into()),
            }
        }
        else if self.buffer.len() >= self.content_length {
            self.content_length = 0;
            let mut buffer = vec![];
            mem::swap(&mut self.buffer, &mut buffer);
            self.http_handler.response(buffer);
        }
    }
}

pub trait HttpHandler {
    fn response(&mut self, data: Vec<u8>);

    fn error(&mut self, _error: io::Error) {
    }
}

pub struct DefaultHttpHandler<MSG, SuccessMsg> {
    actor: Pid<MSG>,
    success_msg: SuccessMsg,
}

impl<MSG, SuccessMsg> DefaultHttpHandler<MSG, SuccessMsg> {
    pub fn new(actor: &Pid<MSG>, success_msg: SuccessMsg) -> Self {
        Self {
            actor: actor.clone(),
            success_msg,
        }
    }
}

impl<MSG, SuccessMsg> HttpHandler for DefaultHttpHandler<MSG, SuccessMsg>
where MSG: Debug,
      SuccessMsg: Fn(Vec<u8>) -> MSG,
{
    fn response(&mut self, data: Vec<u8>) {
        let _ = self.actor.send_message((self.success_msg)(data));
    }
}

pub struct Http {
    process_queue: ProcessQueue,
}

impl Http {
    pub fn new() -> Self {
        Self {
            process_queue: ProcessQueue::new(10, 2),
        }
    }

    pub fn get<HANDLER>(&self, uri: &str, http_handler: HANDLER, event_loop: &EventLoop)
    where HANDLER: HttpHandler + Send + 'static,
    {
        TcpConnection::ip4(&self.process_queue, event_loop, uri, 80, Connection::new(uri, http_handler));
    }
}

impl Default for Http {
    fn default() -> Self {
        Self::new()
    }
}
