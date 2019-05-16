/* Benchmark:
Running 30s test @ http://127.0.0.1:1337/
  12 threads and 400 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency     4.66ms   10.96ms 501.55ms   99.62%
    Req/Sec     7.95k     1.22k   32.02k    94.89%
  2836087 requests in 30.04s, 262.36MB read
Requests/sec:  94408.47
Transfer/sec:      8.73MB
 */

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
    let mut event_loop = Loop::new().expect("event loop");

    TcpListener::ip4(&mut event_loop, "127.0.0.1:1337", Listener {}).expect("listen");

    event_loop.run().expect("event loop run");
}
