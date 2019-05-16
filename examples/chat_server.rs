extern crate mini;

use std::os::unix::io::AsRawFd;
use std::net;

use mini::aio::handler::{
    Handler,
    Loop,
    Stream,
};
use mini::aio::net::{
    TcpConnection,
    TcpConnectionNotify,
    TcpListenNotify,
};
use mini::aio::net::TcpListener;

use self::Msg::*;

enum Msg {
    Accepted(TcpConnection),
    Received(Vec<u8>),
    Closed(TcpConnection),
}

struct ChatHandler {
    clients: Vec<TcpConnection>,
    event_loop: Loop,
}

impl ChatHandler {
    fn new(event_loop: &Loop) -> Self {
        Self {
            clients: vec![],
            event_loop: event_loop.clone(),
        }
    }
}

impl Handler for ChatHandler {
    type Msg = Msg;

    fn update(&mut self, _stream: &Stream<Msg>, msg: Self::Msg) {
        match msg {
            Accepted(tcp_connection) => self.clients.push(tcp_connection),
            Received(data) => {
                for client in &self.clients {
                    if let Err(error) = client.write(data.clone()) {
                        eprintln!("Error send message: {}", error);
                    }
                }
                if data == b"/quit\n" {
                    self.event_loop.stop();
                }
            },
            Closed(tcp_connection) => {
                self.clients.retain(|client| client.as_raw_fd() != tcp_connection.as_raw_fd());
            },
        }
    }
}

struct Listener {
    stream: Stream<Msg>,
}

impl Listener {
    fn new(event_loop: &mut Loop) -> Self {
        let handler = ChatHandler::new(event_loop);
        Self {
            stream: event_loop.spawn(handler),
        }
    }
}

impl TcpListenNotify for Listener {
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
        Box::new(Server::new(&self.stream))
    }
}

struct Server {
    stream: Stream<Msg>,
}

impl Server {
    fn new(stream: &Stream<Msg>) -> Self {
        Self {
            stream: stream.clone(),
        }
    }
}

impl TcpConnectionNotify for Server {
    fn accepted(&mut self, connection: &mut TcpConnection) {
        self.stream.send(Accepted(connection.clone()));
    }

    fn received(&mut self, _connection: &mut TcpConnection, data: Vec<u8>) {
        self.stream.send(Received(data));
    }

    fn closed(&mut self, connection: &mut TcpConnection) {
        self.stream.send(Closed(connection.clone()));
    }
}

fn main() {
    let mut event_loop = Loop::new().expect("event loop");

    let listener = Listener::new(&mut event_loop);
    TcpListener::ip4(&mut event_loop, "127.0.0.1:1337", listener).expect("ip4 listener");

    event_loop.run().expect("event loop run");
}
