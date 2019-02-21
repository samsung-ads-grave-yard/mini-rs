extern crate mini;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use mini::actor::{
    Pid,
    ProcessContinuation,
    ProcessQueue,
    SpawnParameters,
};

use self::Msg1::*;
use self::Msg2::*;
use self::Msg3::*;

#[derive(Debug)]
enum Msg1 {
    Add(i64),
    AddToState,
    Pid2(Pid<Msg2>),
}

#[derive(Debug)]
enum Msg2 {
    Sub(i64),
    SubFromState,
}

#[derive(Debug)]
enum Msg3 {
    Pid1(Pid<Msg1>),
}

fn main() {
    let process_queue = ProcessQueue::new(2, 4);

    let actor_handler3 = |_current: &Pid<_>, msg: Option<Msg3>| {
        match msg {
            Some(Pid1(pid)) => {
                pid.send_message(Add(1)).expect("send message");
                pid.send_message(AddToState).expect("send message");
                ProcessContinuation::Stop
            },
            _ => ProcessContinuation::WaitMessage,
        }
    };

    let sum = Arc::new(AtomicUsize::new(0));
    let state = Arc::clone(&sum);
    let mut state1 = 0;
    let mut pid2 = None;
    let pq = process_queue.clone();
    let actor_handler1 = move |current: &Pid<_>, msg: Option<Msg1>| {
        match msg {
            Some(msg) => {
                match msg {
                    Add(num) => {
                        if num == 5 {
                            let pid3 = pq.blocking_spawn(SpawnParameters {
                                handler: actor_handler3,
                                message_capacity: 2,
                                max_message_per_cycle: 1,
                            });
                            pid3.send_message(Pid1(current.clone())).expect("send message");
                        }
                        state1 += num;
                        ProcessContinuation::WaitMessage
                    },
                    AddToState => {
                        state.fetch_add(state1 as usize, Ordering::SeqCst);
                        ProcessContinuation::Stop
                    },
                    Pid2(pid) => {
                        pid2 = Some(pid);
                        ProcessContinuation::Continue
                    },
                }
            },
            None => {
                if let Some(ref pid) = pid2 {
                    pid.send_message(Sub(35)).expect("send message");
                    pid.send_message(SubFromState).expect("send message");
                }
                ProcessContinuation::WaitMessage
            },
        }
    };

    let pid1 = process_queue.blocking_spawn(SpawnParameters {
        handler: actor_handler1,
        message_capacity: 5,
        max_message_per_cycle: 1,
    });

    let state = Arc::clone(&sum);
    let mut state2 = 0;
    let actor_handler2 = move |current: &Pid<_>, msg: Option<Msg2>| {
        match msg {
            Some(msg) => {
                match msg {
                    Sub(num) => {
                        state2 -= num;
                        pid1.send_message(Add(5)).expect("send message");
                        ProcessContinuation::WaitMessage
                    },
                    SubFromState => {
                        state.fetch_add(state2 as usize, Ordering::SeqCst);
                        ProcessContinuation::Stop
                    },
                }
            },
            None => {
                pid1.send_message(Pid2(current.clone())).expect("send message");
                pid1.send_message(Add(50)).expect("send message");
                ProcessContinuation::WaitMessage
            },
        }
    };

    process_queue.blocking_spawn(SpawnParameters {
        handler: actor_handler2,
        message_capacity: 2,
        max_message_per_cycle: 1,
    });

    process_queue.join();

    println!("Sum: {}", sum.load(Ordering::SeqCst));
}
