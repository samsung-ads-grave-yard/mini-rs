extern crate mini;

use mini::actor::{
    Pid,
    ProcessContinuation,
    ProcessQueue,
    SpawnParameters,
};
use mini::http::Http;

use self::Msg::*;

enum Msg {
    HttpGet(Vec<u8>), // TODO: change to Request.
}

fn main() {
    let process_queue = ProcessQueue::new(10, 2);

    let actor_handler = move |current: &Pid<_>, msg: Option<Msg>| {
        match msg {
            Some(HttpGet(body)) => {
                println!("{}", String::from_utf8_lossy(&body));
                ProcessContinuation::Stop
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

    http.get("crates.io", pid, HttpGet);

    process_queue.join();
}
