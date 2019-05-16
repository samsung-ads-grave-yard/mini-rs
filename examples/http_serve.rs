extern crate mini;

use mini::aio::http_server::{
    self,
    HttpHandler,
    Request,
};
use mini::aio::handler::Loop;

#[derive(Clone)]
struct Http {
}

impl HttpHandler for Http {
    fn request(&mut self, request: &Request) -> String {
        let content = format!("You're on page {} and you queried {} via {}", request.path, request.query_string,
            request.method);
        content
    }
}

fn main() {
    let mut event_loop = Loop::new().expect("event loop");

    http_server::serve(&mut event_loop, "127.0.0.1:1337", Http {}).expect("http serve");

    event_loop.run().expect("event loop run");
}
