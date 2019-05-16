extern crate mini;

use std::io;

use mini::aio::handler::{
    Handler,
    Loop,
    Stream,
};
use mini::aio::http::{
    DefaultHttpHandler,
    Http,
    HttpHandlerIgnoreErr,
};

use self::Msg::*;

#[derive(Debug)]
enum Msg {
    HttpGet(Vec<u8>), // TODO: change to Request.
    HttpError(io::Error),
}

struct HttpHandler {
}

impl Handler for HttpHandler {
    type Msg = Msg;

    fn update(&mut self, _stream: &Stream<Msg>, msg: Self::Msg) {
        match msg {
            HttpGet(body) => {
                println!("{}", String::from_utf8_lossy(&body));
            },
            HttpError(error) => {
                eprintln!("Error: {}", error);
            },
        }
    }
}

fn main() {
    let mut event_loop = Loop::new().expect("event loop");

    let stream = event_loop.spawn(HttpHandler {});

    let http = Http::new();

    http.get("ww.redbook.io", &mut event_loop, DefaultHttpHandler::new(&stream, HttpGet, HttpError));
    http.get("www.redbook.io", &mut event_loop, HttpHandlerIgnoreErr::new(&stream, HttpGet));

    event_loop.run().expect("run");
}
