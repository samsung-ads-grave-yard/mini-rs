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

    fn connected(&mut self, _listener: &TcpListener) -> Box<TcpConnectionNotify> {
        Box::new(Server::new())
    }
}

struct Server {
    clients: Vec<TcpConnection>,
}

impl Server {
    fn new() -> Self {
        Self {
            clients: vec![],
        }
    }
}

impl TcpConnectionNotify for Server {
    fn accepted(&mut self, connection: &mut TcpConnection) {
        println!("Accepted");
        self.clients.push(connection.clone());
    }

    fn received(&mut self, _connection: &mut TcpConnection, data: Vec<u8>) {
        println!("Received {}", String::from_utf8_lossy(&data));
        for client in &self.clients {
            if let Err(error) = client.write(data.clone()) {
                eprintln!("Error send message: {}", error);
            }
        }
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
