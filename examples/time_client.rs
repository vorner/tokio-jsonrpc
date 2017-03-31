// Copyright 2017 tokio-jsonrpc Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

//! A clients that requests the time
//!
//! A client requesting time to a server on localhost:2345.
//! It will inovoke the "now" method, which will return the current
//! unix timestamp (number of seconds since 1.1. 1970).

extern crate tokio_jsonrpc;
extern crate tokio_core;
extern crate tokio_io;
extern crate serde_json;
extern crate futures;
#[macro_use]
extern crate slog;
extern crate slog_term;
extern crate slog_async;


use std::time::Duration;

use futures::Future;
use tokio_core::reactor::Core;
use tokio_core::net::TcpStream;
use tokio_io::AsyncRead;
use slog::{Drain, Logger};

use tokio_jsonrpc::{Endpoint, LineCodec};
use tokio_jsonrpc::message::Response;

fn main() {
    // An application logger
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::CompactFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();

    let logger = Logger::root(drain, o!("app" => "Time client example"));
    info!(logger, "Starting up");
    // Usual setup of async server
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    // Bind a server socket
    let socket = TcpStream::connect(&"127.0.0.1:2345".parse().unwrap(), &handle);

    let client = socket.and_then(|socket| {

        // Create a client endpoint
        let (client, _) = Endpoint::client_only(socket.framed(LineCodec::new()))
            .logger(logger.new(o!("client" => 1)))
            .start(&handle);

        info!(logger, "Calling rpc");
        client.call("now".to_owned(), None, Some(Duration::from_secs(5)))
            .and_then(|(_client, response)| response)
            .map(|x| match x {
                     // Received an error from the server,
                     Some(Response { result: Ok(result), .. }) => {
                let r: u64 = serde_json::from_value(result).unwrap();
                info!(logger, "received response"; "result" => format!("{:?}", r));
            },
                     // Received an error from the server,
                     Some(Response { result: Err(err), .. }) => {
                info!(logger, "remote error"; "error" => format!("{:?}", err));
            },
                     // Timeout
                     None => info!(logger, "timeout"),
                 })
    });

    // Run the whole thing
    core.run(client).unwrap();
}
