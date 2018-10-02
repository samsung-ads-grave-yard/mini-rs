extern crate mini;

use std::io::Write;
use std::net::{TcpListener, TcpStream};

use mini::actor::{
    ProcessQueue,
    SpawnParameters,
};
use mini::async::{
    EventLoop,
    TcpConnectionNotify,
    TcpListenNotify,
};
use mini::async::TcpListener as ActorTcpListener;

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
    fn accepted(&mut self, _connection: &mut TcpStream) {
        println!("Connection accepted.");
    }

    fn received(&mut self, connection: &mut TcpStream, data: Vec<u8>) {
        println!("Data received, looping it back.");
        connection.write(b"server says: "); // TODO: make these writes async.
        connection.write(&data); // TODO: handle errors.
    }

    fn closed(&mut self, _connection: &mut TcpStream) {
        println!("Server closed.");
    }
}

fn main() {
    let process_queue = ProcessQueue::new(20, 4);
    let event_loop = EventLoop::new().expect("event loop");

    process_queue.blocking_spawn(SpawnParameters {
        handler: ActorTcpListener::ip4(&event_loop, Listener {}).expect("ip4 listener"),
        message_capacity: 20,
        max_message_per_cycle: 10,
    });

    event_loop.run();
}
