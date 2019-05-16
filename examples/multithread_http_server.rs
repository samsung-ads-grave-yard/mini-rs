/*
 * Interesting discussion:
 * https://marc.info/?l=linux-api&m=155355980013050&w=2
 *
 * Explanation of limitations of EPOLLEXCLUSIVE: https://patchwork.kernel.org/patch/8224651/
 */

/*
 * Benchmark:
 */

extern crate mini;

use std::mem;
use std::net;
use std::os::unix::io::{
    AsRawFd,
    FromRawFd,
    RawFd,
};
use std::sync::mpsc::{
    SyncSender,
    sync_channel,
};
use std::thread;

use mini::aio::async::Mode;
use mini::aio::handler::Loop;
use mini::aio::net::{
    ListenerMsg::ReadEvent,
    TcpConnection,
    TcpConnectionNotify,
    TcpListener,
    TcpListenNotify,
};

struct Listener {
}

impl Listener {
    fn new() -> Self {
        Self {
        }
    }
}

impl TcpListenNotify for Listener {
    fn connected(&mut self, _listener: &net::TcpListener) -> Box<TcpConnectionNotify> {
        Box::new(Server {})
    }
}

struct MasterListener {
    senders: Vec<SyncSender<RawFd>>,
}

impl MasterListener {
    fn new(senders: Vec<SyncSender<RawFd>>) -> Self {
        Self {
            senders,
        }
    }
}

impl TcpListenNotify for MasterListener {
    fn listening(&mut self, listener: &net::TcpListener) {
        for sender in &self.senders {
            sender.send(listener.as_raw_fd()).expect("send");
        }

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
        let result = connection.write(response.into_bytes()); // TODO: handle errors.
        if let Err(error) = result {
            println!("{}", error);
        }
    }

    fn closed(&mut self, _connection: &mut TcpConnection) {
    }
}

fn main() {
    const NUM_CPU: usize = 5 - 1;
    let mut senders = vec![];

    for cpu in 2..NUM_CPU {
        let (sender, receiver) = sync_channel(1);
        senders.push(sender);
        thread::spawn(move || {
            unsafe {
                let tid = pthread_self();
                let mut set: cpu_set_t = std::mem::zeroed();
                CPU_SET(cpu, &mut set);
                pthread_setaffinity_np(tid, std::mem::size_of::<cpu_set_t>(), &set);
            }

            let fd = receiver.recv().expect("recv");
            println!("Fd: {}", fd);
            let mut event_loop = Loop::new().expect("event loop");

            let tcp_listener = unsafe { net::TcpListener::from_raw_fd(fd) };
            let listener = TcpListener::new(tcp_listener, Listener::new(), &event_loop);
            let stream = event_loop.spawn(listener);
            event_loop.add_raw_fd(fd, Mode::Read, &stream, ReadEvent).expect("add raw fd");

            event_loop.run().expect("event loop run");
        });
    }

    unsafe {
        let tid = pthread_self();
        let mut set: cpu_set_t = std::mem::zeroed();
        CPU_SET(1, &mut set);
        pthread_setaffinity_np(tid, std::mem::size_of::<cpu_set_t>(), &set);
    }

    let mut event_loop = Loop::new().expect("event loop");

    TcpListener::ip4(&mut event_loop, "127.0.0.1:1337", MasterListener::new(senders)).expect("listen");

    event_loop.run().expect("event loop run");
}

#[repr(C)]
pub struct cpu_set_t {
    bits: [u64; 16],
}

fn CPU_SET(cpu: usize, cpuset: &mut cpu_set_t) {
    let size_in_bits = 8 * mem::size_of_val(&cpuset.bits[0]); // 32, 64 etc
    let (idx, offset) = (cpu / size_in_bits, cpu % size_in_bits);
    cpuset.bits[idx] |= 1 << offset;
}

type pthread_t = u64;

extern "C" {
    fn pthread_self() -> pthread_t;

    fn pthread_setaffinity_np(thread: pthread_t, cpusetsize: usize, cpuset: *const cpu_set_t) -> i32;
}
