// Copyright 2017 tokio-jsonrpc Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

extern crate tokio_jsonrpc;
extern crate tokio_core;
extern crate futures;
#[macro_use]
extern crate serde_json;

use std::time::Duration;
use std::io::{Error as IoError, ErrorKind};

use tokio_jsonrpc::{Endpoint, LineCodec, Server, ServerCtl, ServerError};

use futures::{Future, Stream};
use tokio_core::reactor::{Core, Timeout};
use tokio_core::net::{TcpListener, TcpStream};
use tokio_core::io::{Io, Framed};
use serde_json::Value;

struct AnswerServer;

/// A test server
///
/// It expects no parameters and the method to be `"test"` and returns 42 on RPC. It expects
/// `"notif"` as a notification. Both `rpc` and `notification` terminate the server.
impl Server for AnswerServer {
    type Success = u32;
    type RPCCallResult = Result<u32, ServerError>;
    type NotificationResult = Result<(), ()>;
    fn rpc(&self, ctl: &ServerCtl, method: &str, params: &Option<Value>) -> Option<Self::RPCCallResult> {
        ctl.terminate();
        assert_eq!(method, "test");
        assert!(params.is_none());
        Some(Ok(42))
    }
    fn notification(&self, ctl: &ServerCtl, method: &str, params: &Option<Value>) -> Option<Self::NotificationResult> {
        ctl.terminate();
        assert_eq!(method, "notif");
        assert!(params.is_none());
        Some(Ok(()))
    }
}

/// Set up a client and a server
///
/// Create a reactor, set a safety timeout (if the test doesn't finish in 15 seconds, panic) and
/// provide two connected TCP streams. We could use unix socket pair, but that wouldn't work on
/// windows, so we just connect on 127.0.0.1.
fn prepare() -> (Core, Framed<TcpStream, LineCodec>, Framed<TcpStream, LineCodec>) {
    let mut reactor = Core::new().unwrap();
    let handle = reactor.handle();
    // Kill the test if it gets stuck
    let timeout = Timeout::new(Duration::new(15, 0), &handle)
        .unwrap()
        .then(|_| -> Result<(), ()> { panic!("Timeout happened") });
    handle.spawn(timeout);
    // Build two connected TCP streams
    let listener = TcpListener::bind(&"127.0.0.1:0".parse().unwrap(), &handle).unwrap();
    let address = listener.local_addr().unwrap();
    let server_finished = listener.incoming()
        .into_future()
        .then(|result| match result {
            Ok((result, _incoming)) => Ok(result.unwrap().0),
            Err((err, _incoming)) => Err(err),
        });
    let client_finished = TcpStream::connect(&address, &handle);
    // Wait for both of them to be connected
    let (s1, s2) = reactor.run(server_finished.join(client_finished))
        .unwrap();
    // And return everything
    (reactor, s1.framed(LineCodec::new()), s2.framed(LineCodec::new()))
}

/// Single RPC call
///
/// Run both the server and client, send a request and wait for the answer. The server terminates
/// after the first request, so we can wait for both to terminate.
#[test]
fn rpc_answer() {
    let (mut reactor, s1, s2) = prepare();
    let both = {
        // Run in a sub-block, so we drop all the clients, etc.
        let handle = reactor.handle();
        let (_client, _ctl, server_finished) = Endpoint::new(s1, AnswerServer).start(&handle);
        let (client, _ctl, _finished) = Endpoint::client_only(s2).start(&handle);
        let client_finished = client.call("test".to_owned(), None, None)
            .and_then(|(_client, answered)| answered)
            .map(|response| assert_eq!(json!(42), response.unwrap().result.unwrap()));
        server_finished.map_err(|_| IoError::new(ErrorKind::Other, "Canceled"))
            .join(client_finished)
            .map(|_| ())
            .map_err(|e| panic!("{:?}", e))
    };
    reactor.run(both).unwrap();
}

/// Send a notification to the server.
#[test]
fn notification() {
    let (mut reactor, s1, s2) = prepare();
    let both = {
        // Run in a sub-block, so we drop all the clients, etc.
        let handle = reactor.handle();
        let (_client, _ctl, server_finished) = Endpoint::new(s1, AnswerServer).start(&handle);
        let (client, _ctl, _finished) = Endpoint::client_only(s2).start(&handle);
        let client_finished = client.notify("notif".to_owned(), None)
            .and_then(|_client| Ok(()));
        server_finished.map_err(|_| IoError::new(ErrorKind::Other, "Canceled"))
            .join(client_finished)
            .map(|_| ())
            .map_err(|e| panic!("{:?}", e))
    };
    reactor.run(both).unwrap();
}
