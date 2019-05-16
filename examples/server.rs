/*
 * FIXME: might be slower with multiple clients because the executed code is different for
 * different clients, so might have bad icache utilization.
 * TODO: try profiling with --collect-systime=yes option of callgrind.
 */

//extern crate cpuprofiler;
extern crate mini;

use std::net;
use std::time::{Duration, SystemTime};

use mini::aio::handler::Loop;
use mini::aio::net::{
    TcpConnection,
    TcpConnectionNotify,
    TcpListenNotify,
};
use mini::aio::net::TcpListener;

//use cpuprofiler::PROFILER;

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
        Box::new(Server::new())
    }
}

struct Server {
    request_count: i32,
    last_time: SystemTime,
}

impl Server {
    fn new() -> Self {
        Self {
            request_count: 0,
            last_time: SystemTime::now(),
        }
    }
}

impl TcpConnectionNotify for Server {
    fn received(&mut self, connection: &mut TcpConnection, data: Vec<u8>) {
        self.request_count += 1;
        let mut answer = vec![];
        let mut index = 0;
        for byte in data {
            if byte == b'\n' {
                answer.push(b'\n');
                let _ = connection.write(answer); // TODO: handle errors.
                answer = vec![];
                index = 0;
           }
            else if byte == b' ' {
                index += 1;
                if index % 2 == 0 {
                    answer.extend(b" NO");
                }
                else {
                    answer.extend(b" YES");
                }
            }
        }

        if let Ok(duration) = self.last_time.elapsed() {
            if duration >= Duration::from_secs(1) {
                println!("{} request/second", self.request_count);
                self.request_count = 0;
                self.last_time = SystemTime::now();
            }
        }
    }

    /*fn closed(&mut self, _connection: &mut TcpConnection) {
        PROFILER.lock().unwrap().stop().unwrap();
    }*/
}

fn main() {
    //PROFILER.lock().unwrap().start("./my-prof.profile").unwrap();

    let mut event_loop = Loop::new().expect("event loop");

    TcpListener::ip4(&mut event_loop, "127.0.0.1:55447", Listener {}).expect("listen");

    event_loop.run().expect("run");
}
