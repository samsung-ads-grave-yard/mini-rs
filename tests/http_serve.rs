extern crate mini;

use std::thread;

use mini::aio::http::Http;
use mini::aio::http_server::{
    self,
    HttpHandler,
    Request,
};
use mini::aio::handler::Loop;

#[derive(Clone)]
struct HttpServer {
}

impl HttpHandler for HttpServer {
    fn request(&mut self, request: &Request) -> String {
        let content = format!("You're on page {} and you queried {} via {}", request.path, request.query_string,
            request.method);
        content
    }
}

#[test]
fn test_http_client_server() {
    thread::spawn(|| {
        let mut event_loop = Loop::new().expect("event loop");
        http_server::serve(&mut event_loop, "127.0.0.1:1337", HttpServer {}).expect("http serve");
        event_loop.run().expect("event loop run");
    });
    let http = Http::new();
    let body = http.blocking_get("http://127.0.0.1:1337").expect("http get");
    assert_eq!(body, b"You're on page / and you queried  via GET".to_vec());
}
