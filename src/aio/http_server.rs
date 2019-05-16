/*
 * TODO: use a proper URL parser.
 * TODO: do proper HTTP protocol handling.
 */

use std::fmt::{self, Display, Formatter};
use std::io;
use std::net;

use aio::handler::{Loop, Stream};
use aio::net::{
    ListenerMsg,
    TcpConnection,
    TcpConnectionNotify,
    TcpListenNotify,
};
use aio::net::TcpListener;

struct Listener<HANDLER> {
    handler: HANDLER,
}

impl<HANDLER> Listener<HANDLER> {
    fn new(handler: HANDLER) -> Self {
        Self {
            handler,
        }
    }
}

impl<HANDLER: HttpHandler + 'static> TcpListenNotify for Listener<HANDLER> {
    fn listening(&mut self, listener: &net::TcpListener) {
        match listener.local_addr() {
            Ok(address) =>
                println!("Listening on {}:{}.", address.ip(), address.port()),
            Err(error) =>
                eprintln!("Could not get local address: {}.", error),
        }
    }

    fn not_listening(&mut self) {
        eprintln!("Could not listen.");
    }

    fn connected(&mut self, _listener: &net::TcpListener) -> Box<TcpConnectionNotify> {
        Box::new(Server::new(self.handler.clone()))
    }
}

struct Server<HANDLER> {
    handler: HANDLER,
}

impl<HANDLER: HttpHandler> Server<HANDLER> {
    fn new(handler: HANDLER) -> Self {
        Self {
            handler,
        }
    }
}

impl<HANDLER: HttpHandler> TcpConnectionNotify for Server<HANDLER> {
    fn accepted(&mut self, _connection: &mut TcpConnection) {
    }

    fn received(&mut self, connection: &mut TcpConnection, data: Vec<u8>) {
        let request = String::from_utf8(data).unwrap_or_else(|_| String::new());
        let mut lines = request.lines();
        let first_line = lines.next().unwrap_or("GET");
        let mut parts = first_line.split_whitespace();
        let method = parts.next().unwrap_or("GET");
        let url = parts.next().unwrap_or("/");
        let mut url_parts = url.split('?');
        let request = Request {
            method: Method::from_str(method),
            path: url_parts.next().unwrap_or("/").to_string(),
            query_string: url_parts.next().unwrap_or("").to_string(),
        };
        let content = self.handler.request(&request);
        let len = content.len();
        let response = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/html\r\n\r\n{}", len, content);
        let _ = connection.write(response.into_bytes()); // TODO: handle errors.
    }

    fn closed(&mut self, _connection: &mut TcpConnection) {
    }
}

pub enum Method {
    Get,
    Post,
}

impl Method {
    pub fn from_str(method: &str) -> Method {
        match method {
            "POST" => Method::Post,
            "GET" | _ => Method::Get,
        }
    }
}

impl Display for Method {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        let method =
            match *self {
                Method::Get => "GET",
                Method::Post => "POST",
            };
        write!(formatter, "{}", method)
    }
}

pub struct Request {
    pub method: Method,
    pub path: String,
    pub query_string: String,
}

pub trait HttpHandler: Clone {
    fn request(&mut self, request: &Request) -> String;
}

pub fn serve<HANDLER>(event_loop: &mut Loop, addr: &str, handler: HANDLER) -> io::Result<Stream<ListenerMsg>>
where HANDLER: HttpHandler + 'static,
{
    TcpListener::ip4(event_loop, addr, Listener::new(handler))
}
