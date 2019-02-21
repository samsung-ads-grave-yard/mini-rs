extern crate mini;

use mini::actor::{
    Pid,
    ProcessContinuation,
    ProcessQueue,
    SpawnParameters,
};
use mini::async::EventLoop;
use mini::http::Http;

use self::Msg::*;

#[derive(Debug)]
enum Msg {
    HttpGet(Vec<u8>), // TODO: change to Request.
}

fn main() {
    let process_queue = ProcessQueue::new(10, 2);

    let actor_handler = move |current: &Pid<_>, msg: Option<Msg>| {
        match msg {
            Some(HttpGet(body)) => {
                println!("{}", String::from_utf8_lossy(&body));
                ProcessContinuation::WaitMessage
            },
            None => {
                ProcessContinuation::WaitMessage
            },
        }
    };

    let pid = process_queue.blocking_spawn(SpawnParameters {
        handler: actor_handler.clone(),
        message_capacity: 5,
        max_message_per_cycle: 1,
    });

    let http = Http::new();

    let event_loop = EventLoop::new().expect("event loop");

    http.get("www.redbook.io", &event_loop, pid, HttpGet);

    event_loop.run();
}
