extern crate mini;

use std::net;

use mini::aio::handler::Loop;
use mini::aio::net::{
    TcpConnection,
    TcpConnectionNotify,
    TcpListenNotify,
};
use mini::aio::net::TcpListener;

struct Listener {
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
        Box::new(Server {})
    }
}

struct Server {
}

impl TcpConnectionNotify for Server {
    fn accepted(&mut self, _connection: &mut TcpConnection) {
        // TODO: send a message to the connection in a second using a timer.
        println!("Connection accepted.");
    }

    fn received(&mut self, connection: &mut TcpConnection, data: Vec<u8>) {
        println!("Data of size {} received, looping it back.", data.len());
        let _ = connection.write(b"server says: ".to_vec());
        let _ = connection.write(data); // TODO: handle errors.
    }

    fn closed(&mut self, _connection: &mut TcpConnection) {
        println!("Server closed.");
    }
}

fn main() {
    let mut event_loop = Loop::new().expect("event loop");

    TcpListener::ip4(&mut event_loop, "127.0.0.1:1337", Listener {}).expect("listen");

    event_loop.run().expect("run");
}
