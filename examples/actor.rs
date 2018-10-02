extern crate mini;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use mini::actor::{
    Pid,
    ProcessContinuation,
    ProcessQueue,
    SpawnParameters,
};

const MAX_ACTOR_COUNT: usize = 1_000_000;

enum Msg {
    Add,
}

fn main() {
    let sum = Arc::new(AtomicUsize::new(0));
    let state = Arc::clone(&sum);
    let actor_handler = move |current: &Pid<_>, msg: Option<Msg>| {
        match msg {
            Some(_) => {
                state.fetch_add(1, Ordering::SeqCst);
                ProcessContinuation::Stop
            },
            None => {
                if current.send_message(Msg::Add).is_ok() {
                    ProcessContinuation::WaitMessage
                }
                else {
                    ProcessContinuation::Continue
                }
            },
        }
    };

    eprintln!("Spawing 1,000,000 actors");

    let process_queue = ProcessQueue::new(1024, 4);

    for _ in 0..MAX_ACTOR_COUNT {
        process_queue.blocking_spawn(SpawnParameters {
            handler: actor_handler.clone(),
            message_capacity: 2,
            max_message_per_cycle: 1,
        });
    }

    while sum.load(Ordering::SeqCst) < MAX_ACTOR_COUNT {
        thread::yield_now();
    }

    println!("Sum: {}", sum.load(Ordering::SeqCst));
}
