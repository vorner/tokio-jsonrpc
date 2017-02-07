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

use tokio_jsonrpc::{Message, Codec};
use tokio_jsonrpc::message::{Request, Notification};

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
        let jsonized = stream.framed(Codec);
        let (w, r) = jsonized.split();
        let answers = r.filter_map(|message| {
            // TODO: We probably want some more convenient handling, like more handy methods on
            // Message.
            match message {
                Message::Request(Request { ref method, ref params, .. }) => {
                    println!("Got method {}", method);
                    if method == "echo" {
                        Some(message.reply(json!([method, params])))
                    } else {
                        Some(message.error(-32601, "Unknown method".to_owned(), None))
                    }
                },
                Message::Notification(Notification { ref method, .. }) => {
                    println!("Got notification {}", method);
                    None
                },
                Message::Unmatched(_) => Some(message.error(-32600, "Not JSONRPC".to_owned(), None)),
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
