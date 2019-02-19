extern crate mini;

use std::io::Write;
use std::net::TcpListener;

use mini::actor::{
    ProcessQueue,
    SpawnParameters,
};
use mini::async::{
    EventLoop,
    TcpConnection,
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
    fn accepted(&mut self, _connection: &mut TcpConnection) {
        println!("Connection accepted.");
    }

    fn received(&mut self, connection: &mut TcpConnection, data: Vec<u8>) {
        println!("Data of size {} received, looping it back.", data.len());
        let _ = connection.write(b"server says: ".to_vec());
        eprintln!("{:?}", String::from_utf8_lossy(&data));
        let _ = connection.write(data); // TODO: handle errors.
    }

    fn closed(&mut self, _connection: &mut TcpConnection) {
        println!("Server closed.");
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

    event_loop.run();
}
