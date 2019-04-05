extern crate mini;

use std::io;

use mini::handler::{
    Loop,
    Stream,
};
use mini::net::{
    ConnectionMsg::{self, Write},
    TcpConnection,
    TcpConnectionNotify,
};
use mini::stdio::{
    InputNotify,
    Stdin,
};

struct Connection {
}

impl Connection {
    fn new() -> Self {
        Self {
        }
    }
}

impl TcpConnectionNotify for Connection {
    fn connected(&mut self, _connection: &mut TcpConnection) {
        println!("Connected");
    }

    fn connect_failed(&mut self) {
        eprintln!("Connect failed");
    }

    fn error(&mut self, error: io::Error) {
        eprintln!("Error: {}", error);
    }

    fn received(&mut self, _connection: &mut TcpConnection, data: Vec<u8>) {
        match String::from_utf8(data) {
            Ok(text) => print!("-> {}", text),
            Err(error) => println!("Error: did not receive valid UTF-8: {}", error),
        }
    }
}

struct StdinHandler {
    connection: Stream<ConnectionMsg>,
}

impl StdinHandler {
    fn new(connection: Stream<ConnectionMsg>) -> Self {
        Self {
            connection,
        }
    }
}

impl InputNotify for StdinHandler {
    fn received(&mut self, data: Vec<u8>) {
        self.connection.send(Write(data));
    }
}

fn main() {
    let mut event_loop = Loop::new().expect("event loop");

    if let Some(connection) = TcpConnection::ip4(&mut event_loop, "localhost", 1337, Connection::new()) {
        Stdin::new(&mut event_loop, StdinHandler::new(connection)).expect("stdin");

        event_loop.run().expect("run");
    }
}
