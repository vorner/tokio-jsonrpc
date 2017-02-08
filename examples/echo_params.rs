// Copyright 2017 tokio-jsonrpc Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

// TODO: Some comments explaining what is happening

extern crate tokio_jsonrpc;

#[macro_use]
extern crate serde_json;
extern crate futures;
extern crate tokio_core;

use tokio_jsonrpc::{Message, LineCodec};
use tokio_jsonrpc::message::{Notification, Broken};

use futures::{Future, Sink, Stream};
use tokio_core::reactor::Core;
use tokio_core::net::TcpListener;
use tokio_core::io::Io;

fn main() {
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let listener = TcpListener::bind(&"127.0.0.1:2345".parse().unwrap(), &handle).unwrap();
    let connections = listener.incoming();
    let service = connections.for_each(|(stream, _)| {
        let jsonized = stream.framed(LineCodec);
        let (w, r) = jsonized.split();
        let answers = r.filter_map(|message| {
            println!("A message received: {:?}", message);
            // TODO: We probably want some more convenient handling, like more handy methods on
            // Message.
            match message {
                Ok(Message::Request(ref req)) => {
                    println!("Got method {}", req.method);
                    if req.method == "echo" {
                        Some(req.reply(json!([req.method, req.params])))
                    } else {
                        Some(req.error(-32601, format!("Unknown method {}", req.method), None))
                    }
                },
                Ok(Message::Notification(Notification { ref method, .. })) => {
                    println!("Got notification {}", method);
                    None
                },
                Err(Broken::Unmatched(_)) => Some(Message::error(-32600, "Not a JSONRPC message".to_owned(), None)),
                Err(Broken::SyntaxError(ref e)) => Some(Message::error(-32700, e.to_owned(), None)),
                _ => None,
            }
        });
        let sent = w.send_all(answers)
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
