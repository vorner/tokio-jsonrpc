// Copyright 2017 tokio-jsonrpc Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

//! A server that responds with the current time
//!
//! A server listening on localhost:2345. It reponds to the „now“ method, returning the current
//! unix timestamp (number of seconds since 1.1. 1970). You can also subscribe to periodic time
//! updates.

#[macro_use]
extern crate tokio_jsonrpc;
extern crate tokio_core;
extern crate tokio_io;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate futures;
#[macro_use]
extern crate slog;
extern crate slog_term;

use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::io;

use futures::{Future, Stream};
use tokio_core::reactor::{Core, Handle, Interval};
use tokio_core::net::TcpListener;
use tokio_io::AsyncRead;
use serde_json::Value;
use slog::{Drain, Logger};
use slog_term::{FullFormat, PlainSyncDecorator};

use tokio_jsonrpc::{Endpoint, LineCodec, Params, RpcError, Server, ServerCtl};

/// Number of seconds since epoch
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// The server implementation
///
/// Future versions of the library will have some helpers to create them in an easier way.
struct TimeServer(Handle, Logger);

impl Server for TimeServer {
    /// The return type of RPC
    ///
    /// As we have two different RPCs with different results, we use the generic Value.
    type Success = Value;
    type RpcCallResult = Result<Value, RpcError>;
    /// Just a formality, we don't need this one
    type NotificationResult = Result<(), ()>;
    /// The actual implementation of the RPC methods
    fn rpc(&self, ctl: &ServerCtl, method: &str, params: &Option<Params>)
           -> Option<Self::RpcCallResult> {
        match method {
            // Return the number of seconds since epoch (eg. unix timestamp)
            "now" => {
                debug!(self.1, "Providing time");
                Some(Ok(Value::Number(now().into())))
            },
            // Subscribe to receiving updates of time (we don't do unsubscription)
            "subscribe" => {
                debug!(self.1, "Subscribing");
                // Some parsing and bailing out on errors
                // XXX: why is params borrowed?
                let params2 = params.clone();
                let params = parse_params!(params2, { secs: u64,
                                                      #[serde(default)]
                                                      nsecs: u32, });
                // XXX: this is not happy code
                //      `impl Carrier` might make it nicer, if/when it lands on stable
                let params = match params {
                    /// XXX: this should not be here, we should not return option by default
                    None => return Some(Err(RpcError::invalid_params(None))),
                    Some(Err(err)) => return Some(Err(err)),
                    Some(Ok(params)) => params,
                };
                // We need to have a client to be able to send notifications
                let client = ctl.client();
                let handle = self.0.clone();
                let logger = self.1.clone();
                // Get a stream that „ticks“
                let result = Interval::new(Duration::new(params.secs, params.nsecs), &self.0)
                    .or_else(|e| Err(RpcError::server_error(Some(format!("Interval: {}", e)))))
                    .map(move |interval| {
                        let logger_cloned = logger.clone();
                        // And send the notification on each tick (and pass the client through)
                        let notified = interval.fold(client, move |client, _| {
                                debug!(logger_cloned, "Tick");
                                client.notify("time".to_owned(), params!([now()]))
                            })
                            // So it can be spawned, spawn needs ().
                            .map(|_| ())
                            // TODO: This reports a „Shouldn't happen“ error ‒ do something
                            // about that
                            .map_err(move |e| {
                                         error!(logger, "Error notifying about a time: {}", e);
                                     });
                        handle.spawn(notified);
                        // We need some result, but we don't send any meaningful value
                        Value::Null
                    });
                Some(result)
            },
            // Method not known
            _ => None,
        }
    }
}

fn main() {
    // An application logger
    let plain = PlainSyncDecorator::new(io::stdout());
    let drain = FullFormat::new(plain).build().fuse();
    let logger = Logger::root(drain, o!("app" => "Time server example"));
    info!(logger, "Starting up");
    // Usual setup of async server
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let listener = TcpListener::bind(&"127.0.0.1:2345".parse().unwrap(), &handle).unwrap();
    let service = listener.incoming()
        .for_each(move |(connection, addr)| {
            let addr = format!("{}", addr);
            // Once a connection is made, create an endpoint on it, using the above server
            let (_client, finished) = Endpoint::new(connection.framed(LineCodec::new()),
                                                    TimeServer(handle.clone(),
                                                               logger.new(o!("cli" => addr.clone(),
                                                                             "context" => "time"))))
                    .logger(logger.new(o!("cli" => addr.clone(), "context" => "json RPC")))
                    .start(&handle);
            // If it finishes with an error, report it
            let logger = logger.clone();
            let err_report =
                finished.map_err(move |e| error!(logger, "Problem on client {}: {}", addr, e));
            handle.spawn(err_report);
            // Just so the for_each is happy, nobody actually uses this
            Ok(())
        });
    // Run the whole thing
    core.run(service).unwrap();
}
