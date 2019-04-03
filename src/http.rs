// TODO: make a web crawler example.

use std::collections::VecDeque;
use std::fmt::Debug;
use std::io;
use std::mem;

use async::EventLoop;
use handler::Stream;
use net::{
    TcpConnection,
    TcpConnectionNotify,
};

#[derive(Clone)]
struct Connection<HANDLER> {
    buffer: VecDeque<u8>,
    content_length: usize,
    http_handler: HANDLER,
    uri: String,
}

impl<HANDLER> Connection<HANDLER> {
    fn new(uri: &str, http_handler: HANDLER) -> Self {
        Self {
            buffer: VecDeque::new(),
            content_length: 0,
            http_handler,
            uri: uri.to_string(),
        }
    }
}

fn deque_compare(buffer: &VecDeque<u8>, start: usize, len: usize, value: &[u8]) -> bool {
    if value.len() < len {
        return false;
    }
    let mut index = 0;
    for i in start..start + len {
        if buffer[i] != value[index] {
            return false;
        }
        index += 1;
    }
    true
}

fn parse_num(buffer: &VecDeque<u8>, start: usize, len: usize) -> Option<usize> {
    let mut result = 0;
    for i in start..start + len {
        if buffer[i] >= b'0' && buffer[i] <= b'9' {
            result *= 10;
            result += (buffer[i] - b'0') as usize;
        }
        else if result != 0 && buffer[i] != b' ' {
            return None;
        }
    }
    Some(result)
}

fn parse_headers(buffer: &VecDeque<u8>) -> Option<usize> {
    // TODO: parse other headers.
    let mut start = 0;
    for i in 0..buffer.len() {
        if buffer[i] == b'\n' {
            let text = b"Content-Length:";
            let end = start + text.len();
            if deque_compare(buffer, start, text.len(), text) {
                let num = parse_num(buffer, end, i - 1 - end); // - 1 to remove the \n.
                return num;
            }
            start = i + 1;
        }
    }
    None
}

fn remove_until_boundary(buffer: &mut VecDeque<u8>) {
    let mut index = buffer.len() - 1;
    for i in 0..buffer.len() {
        if i + 4 <= buffer.len() && deque_compare(&buffer, i, 4, b"\r\n\r\n") {
            index = i + 4;
            break;
        }
    }
    for _ in 0..index {
        buffer.pop_front();
    }
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

    fn received(&mut self, connection: &mut TcpConnection, data: Vec<u8>) {
        self.buffer.extend(data);
        if self.content_length == 0 {
            match parse_headers(&self.buffer) {
                Some(content_length) => {
                    remove_until_boundary(&mut self.buffer);
                    self.content_length = content_length;
                },
                None => (), // Might find the content length in the next data.
            }
        }
        else if self.buffer.len() >= self.content_length {
            let buffer = mem::replace(&mut self.buffer, VecDeque::new());
            self.http_handler.response(buffer.into());
            connection.dispose();
        }
    }
}

pub trait HttpHandler {
    fn response(&mut self, data: Vec<u8>);

    fn error(&mut self, _error: io::Error) {
    }
}

pub struct DefaultHttpHandler<MSG, SuccessMsg> {
    handle: Stream<MSG>,
    success_msg: SuccessMsg,
}

impl<MSG, SuccessMsg> DefaultHttpHandler<MSG, SuccessMsg> {
    pub fn new(handle: &Stream<MSG>, success_msg: SuccessMsg) -> Self {
        Self {
            handle: handle.clone(),
            success_msg,
        }
    }
}

impl<MSG, SuccessMsg> HttpHandler for DefaultHttpHandler<MSG, SuccessMsg>
where MSG: Debug,
      SuccessMsg: Fn(Vec<u8>) -> MSG,
{
    fn response(&mut self, data: Vec<u8>) {
        //let _ = self.actor.send_message((self.success_msg)(data));
    }
}

pub struct Http {
}

impl Http {
    pub fn new() -> Self {
        Self {
        }
    }

    pub fn get<HANDLER>(&self, uri: &str, http_handler: HANDLER, event_loop: &EventLoop)
    where HANDLER: HttpHandler + Send + 'static,
    {
        TcpConnection::ip4(event_loop, uri, 80, Connection::new(uri, http_handler));
    }
}

impl Default for Http {
    fn default() -> Self {
        Self::new()
    }
}
