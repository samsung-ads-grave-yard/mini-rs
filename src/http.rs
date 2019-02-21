// TODO: maybe take inspiration from: https://www.monkeysnatchbanana.com/2015/12/19/inside-the-pony-tcp-stack/
// TODO: make a web crawler example.

use std::fmt::Debug;
use std::mem;
use std::os::unix::io::RawFd;

use actor::{
    Pid,
    ProcessContinuation,
    ProcessQueue,
    SpawnParameters,
};
use async::{
    EventLoop,
    TcpConnection,
    TcpConnectionNotify,
};

use self::Msg::*;

pub enum Msg {
    Connected(RawFd),
}

#[derive(Clone)]
struct Connection<M, MSG> {
    buffer: Vec<u8>,
    content_length: usize,
    message: M,
    receiver: Pid<MSG>,
    uri: String,
}

impl<M, MSG> Connection<M, MSG> {
    fn new(uri: &str, receiver: Pid<MSG>, message: M) -> Self {
        Self {
            buffer: vec![],
            content_length: 0,
            message,
            receiver,
            uri: uri.to_string(),
        }
    }
}

fn parse_headers(buffer: &[u8]) -> usize {
    // TODO: parse other headers.
    let mut size = 0;
    for line in buffer.split(|byte| *byte == b'\n') {
        if line.starts_with(b"Content-Length:") {
            let mut parts = line.split(|byte| *byte == b':');
            parts.next().expect("header name");
            let mut value = String::from_utf8_lossy(parts.next().expect("header value"));
            size = str::parse(value.trim()).expect("content length is not a number");
        }
    }
    size
}

impl<M, MSG> TcpConnectionNotify for Connection<M, MSG>
where M: Fn(Vec<u8>) -> MSG,
      MSG: Debug,
{
    fn connecting(&mut self, _connection: &mut TcpConnection, count: u32) {
        println!("Connecting. Attempt #{}", count);
    }

    fn connected(&mut self, connection: &mut TcpConnection) {
        connection.write(format!("GET / HTTP/1.1\r\nHost: {}\r\n\r\n", self.uri).into_bytes()).expect("write");
    }

    fn received(&mut self, _connection: &mut TcpConnection, data: Vec<u8>) {
        self.buffer.extend(data);
        if self.buffer.ends_with(b"\r\n\r\n") {
            self.content_length = parse_headers(&self.buffer);
            let mut buffer = vec![];
            mem::swap(&mut self.buffer, &mut buffer);
        }
        else if self.buffer.len() >= self.content_length {
            self.content_length = 0;
            let mut buffer = vec![];
            mem::swap(&mut self.buffer, &mut buffer);
            self.receiver.send_message((self.message)(buffer)).expect("send message");
        }
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

    pub fn get<M, MSG>(&self, uri: &str, event_loop: &EventLoop, receiver: Pid<MSG>, message: M)
    where M: Fn(Vec<u8>) -> MSG + Send + 'static,
          MSG: Debug + Send + 'static,
    {
        TcpConnection::ip4(&self.process_queue, event_loop, uri, 80, Connection::new(uri, receiver, message));
    }
}
