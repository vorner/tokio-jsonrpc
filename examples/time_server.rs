// Copyright 2017 tokio-jsonrpc Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

//! A server that responds with the current time
//!
//! A server listening on localhost:2345. It reponds to the „now“ method, returning the current
//! unix timestamp (number of seconds since 1.1. 1970). You cal also subscribe to periodic time
//! updates.

extern crate tokio_jsonrpc;
extern crate tokio_core;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;
extern crate futures;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio_jsonrpc::{Endpoint, LineCodec, Server, ServerCtl, RPCError};

use futures::{Future, Stream};
use tokio_core::io::Io;
use tokio_core::reactor::{Handle, Interval, Core};
use tokio_core::net::TcpListener;
use serde_json::{Value, from_value};

/// A helper struct to deserialize the parameters
#[derive(Deserialize)]
struct SubscribeParams {
    secs: u64,
    #[serde(default)]
    nsecs: u32,
}

/// Number of seconds since epoch
fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

/// The server implementation
///
/// Future versions of the library will have some helpers to create them in an easier way.
struct TimeServer(Handle);

impl Server for TimeServer {
    /// The return type of RPC
    ///
    /// As we have two different RPCs with different results, we use the generic Value.
    type Success = Value;
    type RPCCallResult = Result<Value, RPCError>;
    /// Just a formality, we don't need this one
    type NotificationResult = Result<(), ()>;
    /// The actual implementation of the RPC methods
    fn rpc(&self, ctl: &ServerCtl, method: &str, params: &Option<Value>) -> Option<Self::RPCCallResult> {
        match method {
            // Return the number of seconds since epoch (eg. unix timestamp)
            "now" => Some(Ok(Value::Number(now().into()))),
            // Subscribe to receiving updates of time (we don't do unsubscription)
            "subscribe" => {
                // Some parsing and bailing out on errors
                if params.is_none() {
                    return Some(Err(RPCError::invalid_params()));
                }
                let s_params = match from_value::<SubscribeParams>(params.clone().unwrap()) {
                    Ok(p) => p,
                    Err(_) => return Some(Err(RPCError::invalid_params())),
                };
                // We need to have a client to be able to send notifications
                let client = ctl.client();
                let handle = self.0.clone();
                // Get a stream that „ticks“
                let result = Interval::new(Duration::new(s_params.secs, s_params.nsecs), &self.0)
                    .or_else(|e| Err(RPCError::server_error(Some(format!("Can't start interval: {}", e)))))
                    .map(move |interval| {
                        // And send the notification on each tick (and pass the client through)
                        let notified = interval.fold(client, |client, _| {
                                client.notify("time".to_owned(), Some(json!([now()])))
                            })
                            // So it can be spawned, spawn needs ().
                            .map(|_| ())
                            // TODO: This reports a „Shouldn't happen“ error ‒ do something about
                            // that
                            .map_err(|e| println!("Error notifying: {}", e));
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
    // Usual setup of async server
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let listener = TcpListener::bind(&"127.0.0.1:2345".parse().unwrap(), &handle).unwrap();
    let service = listener.incoming()
        .for_each(move |(connection, addr)| {
            // Once a connection is made, create an endpoint on it, using the above server
            let (_client, finished) = Endpoint::new(connection.framed(LineCodec::new()), TimeServer(handle.clone())).start(&handle);
            // If it finishes with an error, report it
            let err_report = finished.map_err(move |e| println!("Problem on client {}: {}", addr, e));
            handle.spawn(err_report);
            // Just so the for_each is happy, nobody actually uses this
            Ok(())
        });
    // Run the whole thing
    core.run(service).unwrap();
}
