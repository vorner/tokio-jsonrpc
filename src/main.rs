extern crate tokio_jsonrpc;

extern crate futures;
extern crate tokio_core;
extern crate serde_json;

use tokio_jsonrpc::{Message, Codec};

use std::io::Error;

use futures::{Future, Sink, Stream};
use futures::stream::{once, Once};
use tokio_core::reactor::Core;
use tokio_core::net::TcpListener;
use tokio_core::io::Io;
use serde_json::Value;

fn main() {
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let listener = TcpListener::bind(&"127.0.0.1:2345".parse().unwrap(), &handle).unwrap();
    let connections = listener.incoming();
    let service = connections.for_each(|(stream, _)| {
        let jsonized = stream.framed(Codec);
        let (w, r) = jsonized.split();
        let header: Once<_, Error> = once(Ok(Message::Batch(vec![Message::request("Hello".to_owned(), Some(Value::Null)),
                                                                 Message::Unmatched(Value::Null).error(42, "Wrong!".to_owned(), None),
                                                                 Message::notification("Alert!".to_owned(), Some(Value::String("blabla".to_owned()))),
                                                                 Message::Unmatched(Value::String(String::new()))])));
        let sent = w.send_all(header)
            .and_then(|(sink, _)| sink.send_all(r))
            .map(|_| ())
            .map_err(|_| {
                // TODO Something with the error ‒ logging?
                ()
            });
        // Do the sending in the background
        handle.spawn(sent);
        Ok(())
    });
    core.run(service).unwrap();
}
