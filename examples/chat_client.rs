extern crate mini;

use std::io::{self, BufRead, stdin};

use mini::actor::ProcessQueue;
use mini::async::EventLoop;
use mini::net::{
    ConnectionMsg::Write,
    TcpConnection,
    TcpConnectionNotify,
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
            Ok(text) => println!("{}", text),
            Err(error) => println!("Error: did not receive valid UTF-8: {}", error),
        }
    }
}

fn main() {
    let process_queue = ProcessQueue::new(20, 4);
    let event_loop = EventLoop::new().expect("event loop");

    let connection = TcpConnection::ip4(&process_queue, &event_loop, "localhost", 1337, Connection::new());

    std::thread::spawn(move || {
        let stdin = stdin();
        let stdin = stdin.lock();

        for line in stdin.lines() {
            let line = line.expect("read line");
            println!("Sending Write");
            connection.send_message(Write(line.into_bytes()));
        }
    });

    event_loop.run().expect("run");
}
