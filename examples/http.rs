extern crate mini;

use std::io;

use mini::actor::{
    Pid,
    ProcessContinuation,
    ProcessQueue,
    SpawnParameters,
};
use mini::async::EventLoop;
use mini::http::{
    DefaultHttpHandler,
    Http,
    HttpHandler,
};

use self::Msg::*;

#[derive(Debug)]
enum Msg {
    HttpGet(Vec<u8>), // TODO: change to Request.
    HttpError(io::Error),
}

struct Handler {
    actor: Pid<Msg>,
}

impl Handler {
    fn new(actor: &Pid<Msg>) -> Self {
        Self {
            actor: actor.clone(),
        }
    }
}

impl HttpHandler for Handler {
    fn response(&mut self, data: Vec<u8>) {
        self.actor.send_message(HttpGet(data)).expect("send message");
    }

    fn error(&mut self, error: io::Error) {
        self.actor.send_message(HttpError(error)).expect("send message");
    }
}

fn main() {
    let process_queue = ProcessQueue::new(10, 2);

    let actor_handler = move |_current: &Pid<_>, msg: Option<Msg>| {
        match msg {
            Some(msg) =>
                match msg {
                    HttpGet(body) => {
                        println!("{}", String::from_utf8_lossy(&body));
                        ProcessContinuation::WaitMessage
                    },
                    HttpError(error) => {
                        eprintln!("Error: {}", error);
                        ProcessContinuation::WaitMessage
                    },
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

    http.get("ww.redbook.io", Handler::new(&pid), &event_loop);
    http.get("www.redbook.io", DefaultHttpHandler::new(&pid, HttpGet), &event_loop);

    event_loop.run().expect("run");
}
