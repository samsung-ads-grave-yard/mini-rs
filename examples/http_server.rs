extern crate mini;

use std::net::TcpListener;

use mini::actor::{
    ProcessQueue,
    SpawnParameters,
};
use mini::async::EventLoop;
use mini::net::{
    TcpConnection,
    TcpConnectionNotify,
    TcpListenNotify,
};
use mini::net::TcpListener as ActorTcpListener;

struct Listener {
}

impl TcpListenNotify for Listener {
    fn listening(&mut self, listener: &TcpListener) {
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

    fn connected(&mut self, _listener: &TcpListener) -> Box<TcpConnectionNotify + Send> {
        Box::new(Server {})
    }
}

struct Server {
}

impl TcpConnectionNotify for Server {
    fn accepted(&mut self, _connection: &mut TcpConnection) {
    }

    fn received(&mut self, connection: &mut TcpConnection, data: Vec<u8>) {
        let request = String::from_utf8(data).unwrap_or_else(|_| String::new());
        let mut lines = request.lines();
        let first_line = lines.next().unwrap_or("GET");
        let mut parts = first_line.split_whitespace();
        let _method = parts.next().unwrap_or("GET");
        let url = parts.next().unwrap_or("/");
        let mut url_parts = url.split('?');
        let path = url_parts.next().unwrap_or("/");
        let query_string = url_parts.next().unwrap_or("");
        let content = format!("You're on page {} and you queried {}", path, query_string);
        let len = content.len();
        let response = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: text/html\r\n\r\n{}", len, content);
        let _ = connection.write(response.into_bytes()); // TODO: handle errors.
    }

    fn closed(&mut self, _connection: &mut TcpConnection) {
    }
}

fn main() {
    let process_queue = ProcessQueue::new(20, 4);
    let event_loop = EventLoop::new().expect("event loop");

    process_queue.blocking_spawn(SpawnParameters {
        handler: ActorTcpListener::ip4(&event_loop, "127.0.0.1:1337", Listener {}).expect("ip4 listener"),
        message_capacity: 20,
        max_message_per_cycle: 10,
    });

    event_loop.run().expect("event loop run");
}
