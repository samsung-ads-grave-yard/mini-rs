// TODO: make a web crawler example.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt::Debug;
use std::io;
use std::mem;
use std::rc::Rc;

use aio::handler::{
    Handler,
    Loop,
    Stream,
};
use aio::net::{
    TcpConnection,
    TcpConnectionNotify,
};
use aio::uhttp_uri::HttpUri;

use self::Msg::*;

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

#[derive(Clone)]
struct Connection<HANDLER> {
    buffer: VecDeque<u8>,
    content_length: usize,
    handler: HANDLER,
    host: String,
    method: &'static str,
    path: String,
}

impl<HANDLER> Connection<HANDLER> {
    fn new(host: &str, handler: HANDLER, path: &str, method: &'static str) -> Self {
        Self {
            buffer: VecDeque::new(),
            content_length: 0,
            handler,
            host: host.to_string(),
            method,
            path: path.to_string(),
        }
    }
}

impl<HANDLER> TcpConnectionNotify for Connection<HANDLER>
where HANDLER: HttpHandler,
{
    fn connecting(&mut self, _connection: &mut TcpConnection, count: u32) {
        println!("Connecting. Attempt #{}", count);
    }

    fn connected(&mut self, connection: &mut TcpConnection) {
        if let Err(error) = connection.write(format!("{} {} HTTP/1.1\r\nHost: {}\r\n\r\n", self.method, self.path,
            self.host).into_bytes())
        {
            self.handler.error(error);
        }
    }

    fn error(&mut self, error: io::Error) {
        self.handler.error(error);
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
        if self.buffer.len() >= self.content_length {
            let buffer = mem::replace(&mut self.buffer, VecDeque::new());
            self.handler.response(buffer.into());
            connection.dispose();
        }
    }
}

pub trait HttpHandler {
    fn response(&mut self, data: Vec<u8>);

    fn error(&mut self, _error: io::Error) {
    }
}

pub struct DefaultHttpHandler<ErrorMsg, MSG, SuccessMsg> {
    error_msg: ErrorMsg,
    stream: Stream<MSG>,
    success_msg: SuccessMsg,
}

impl<ErrorMsg, MSG, SuccessMsg> DefaultHttpHandler<ErrorMsg, MSG, SuccessMsg> {
    pub fn new(stream: &Stream<MSG>, success_msg: SuccessMsg, error_msg: ErrorMsg) -> Self {
        Self {
            error_msg,
            stream: stream.clone(),
            success_msg,
        }
    }
}

impl<ErrorMsg, MSG, SuccessMsg> HttpHandler for DefaultHttpHandler<ErrorMsg, MSG, SuccessMsg>
where MSG: Debug,
      ErrorMsg: Fn(io::Error) -> MSG,
      SuccessMsg: Fn(Vec<u8>) -> MSG,
{
    fn error(&mut self, error: io::Error) {
        self.stream.send((self.error_msg)(error));
    }

    fn response(&mut self, data: Vec<u8>) {
        self.stream.send((self.success_msg)(data));
    }
}

pub struct HttpHandlerIgnoreErr<MSG, SuccessMsg> {
    stream: Stream<MSG>,
    success_msg: SuccessMsg,
}

impl<MSG, SuccessMsg> HttpHandlerIgnoreErr<MSG, SuccessMsg> {
    pub fn new(stream: &Stream<MSG>, success_msg: SuccessMsg) -> Self {
        Self {
            stream: stream.clone(),
            success_msg,
        }
    }
}

impl<MSG, SuccessMsg> HttpHandler for HttpHandlerIgnoreErr<MSG, SuccessMsg>
where MSG: Debug,
      SuccessMsg: Fn(Vec<u8>) -> MSG,
{
    fn response(&mut self, data: Vec<u8>) {
        self.stream.send((self.success_msg)(data));
    }
}

pub struct Http {
}

impl Http {
    pub fn new() -> Self {
        Self {
        }
    }

    fn blocking<F: Fn(Rc<RefCell<io::Result<Vec<u8>>>>, &mut Loop) -> io::Result<()>>(&self, callback: F) -> io::Result<Vec<u8>> {
        let result = Rc::new(RefCell::new(Ok(vec![])));
        let mut event_loop = Loop::new()?;
        callback(result.clone(), &mut event_loop)?;
        event_loop.run()?;
        let mut result = result.borrow_mut();
        mem::replace(&mut *result, Ok(vec![]))
    }

    pub fn blocking_get(&self, uri: &str) -> io::Result<Vec<u8>> {
        self.blocking(|result, event_loop| {
            let stream = event_loop.spawn(BlockingHttpHandler::new(&event_loop, result));
            let http = Http::new();
            http.get(uri, event_loop, DefaultHttpHandler::new(&stream, HttpGet, HttpError))
                .map_err(|()| io::Error::new(io::ErrorKind::Other, ""))
        })
    }

    pub fn blocking_post(&self, uri: &str) -> io::Result<Vec<u8>> {
        self.blocking(|result, event_loop| {
            let stream = event_loop.spawn(BlockingHttpHandler::new(&event_loop, result));
            let http = Http::new();
            http.post(uri, event_loop, DefaultHttpHandler::new(&stream, HttpGet, HttpError))
                .map_err(|()| io::Error::new(io::ErrorKind::Other, ""))
        })
    }

    pub fn get<HANDLER>(&self, uri: &str, event_loop: &mut Loop, handler: HANDLER) -> Result<(), ()>
    where HANDLER: HttpHandler + 'static,
    {
        let uri = HttpUri::new(uri)?;
        TcpConnection::ip4(event_loop, uri.host, uri.port, Connection::new(uri.host, handler, uri.resource.path, "GET"));
        Ok(())
    }

    pub fn post<HANDLER>(&self, uri: &str, event_loop: &mut Loop, handler: HANDLER) -> Result<(), ()>
    where HANDLER: HttpHandler + 'static,
    {
        let uri = HttpUri::new(uri)?;
        TcpConnection::ip4(event_loop, uri.host, uri.port, Connection::new(uri.host, handler, uri.resource.path, "POST"));
        Ok(())
    }
}

impl Default for Http {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
enum Msg {
    HttpGet(Vec<u8>),
    HttpError(io::Error),
}

struct BlockingHttpHandler {
    event_loop: Loop,
    result: Rc<RefCell<io::Result<Vec<u8>>>>,
}

impl BlockingHttpHandler {
    fn new(event_loop: &Loop, result: Rc<RefCell<io::Result<Vec<u8>>>>) -> Self {
        Self {
            event_loop: event_loop.clone(),
            result,
        }
    }
}

impl Handler for BlockingHttpHandler {
    type Msg = Msg;

    fn update(&mut self, _stream: &Stream<Msg>, msg: Self::Msg) {
        match msg {
            HttpGet(body) => {
                *self.result.borrow_mut() = Ok(body);
            },
            HttpError(error) => {
                *self.result.borrow_mut() = Err(error);
            },
        }
        self.event_loop.stop()
    }
}
