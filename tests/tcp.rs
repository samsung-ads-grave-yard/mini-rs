extern crate mini;

use std::io::Write;
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use mini::actor::{
    ProcessQueue,
    SpawnParameters,
};
use mini::async::{
    EpollResult,
    EventLoop,
    TcpConnection,
    TcpConnectionNotify,
    TcpListenNotify,
    event_list,
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
        let _ = connection.write(data); // TODO: handle errors.
    }

    fn closed(&mut self, _connection: &mut TcpConnection) {
        println!("Server closed.");
    }
}

#[test]
fn test_blocked_write() {
    let process_queue = ProcessQueue::new(20, 4);
    let event_loop = EventLoop::new().expect("event loop");

    process_queue.blocking_spawn(SpawnParameters {
        handler: ActorTcpListener::ip4(&event_loop, "127.0.0.1:1337", Listener {}).expect("ip4 listener"),
        message_capacity: 20,
        max_message_per_cycle: 10,
    });

    let done = Arc::new(AtomicBool::new(false));

    let thread_done = done.clone();
    thread::spawn(move || {
        use std::io::Read;
        use std::net::TcpStream;

        let mut stream = TcpStream::connect("localhost:1337").expect("stream");

        let mut buffer = vec![];
        let text: Vec<u8> = b"hello".iter().cycle().cloned().take(1000).collect();
        for i in 0..10_000 {
            stream.write_all(&text);
            if i % 1000 == 0 {
                let mut temp_buffer = vec![0u8; 1000];
                let read = stream.read(&mut temp_buffer).expect("read");
                buffer.extend(temp_buffer.drain(..));
            }
        }

        while buffer.len() < 10_000_000 {
            let mut temp_buffer = vec![0u8; 1000];
            let read = stream.read(&mut temp_buffer).expect("read");
            buffer.extend(temp_buffer.drain(..));
        }

        thread_done.store(true, Ordering::SeqCst);
    });

    let mut event_list = event_list();

    while !done.load(Ordering::SeqCst) {
        match event_loop.iterate(&mut event_list) {
            // Restart if interrupted by signal.
            EpollResult::Interrupted => continue,
            EpollResult::Error(error) => panic!("{}", error),
            EpollResult::Ok => (),
        }
    }
}