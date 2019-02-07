// TODO: maybe take inspiration from: https://www.monkeysnatchbanana.com/2015/12/19/inside-the-pony-tcp-stack/
// TODO: make a web crawler example.

use std::os::unix::io::RawFd;
use std::sync::Arc;

use actor::{
    Pid,
    ProcessContinuation,
    ProcessQueue,
    SpawnParameters,
};

use net::tcp::connect_to_host;
use self::Msg::*;

pub enum Msg {
    Connected(RawFd),
}

pub struct Http {
    process_queue: ProcessQueue,
}

impl Http {
    pub fn new() -> Self {
        Self {
            process_queue: ProcessQueue::new(10, 2),
        }
    }

    pub fn get<M, MSG>(&self, uri: &str, receiver: Pid<MSG>, message: M)
    where M: Fn(Vec<u8>) -> MSG
    {
        let handler = |_current: &Pid<_>, msg: Option<Msg>| {
            ProcessContinuation::WaitMessage
        };
        let pid = self.process_queue.blocking_spawn(SpawnParameters {
            handler,
            message_capacity: 2,
            max_message_per_cycle: 1,
        });

        //connect_to_host(uri, "80", &self.process_queue, &pid, Connected);
    }
}
