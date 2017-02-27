// Copyright 2017 tokio-jsonrpc Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

extern crate tokio_jsonrpc;
extern crate tokio_core;
extern crate futures;
extern crate relay;
#[macro_use]
extern crate serde_json;

use std::time::Duration;
use std::io::{Error as IoError};
use std::cell::Cell;
use std::rc::Rc;

use tokio_jsonrpc::{Endpoint, LineCodec, Client, Server, ServerCtl, ServerError};
use tokio_jsonrpc::message::RPCError;

use futures::{Future, Stream};
use futures::future::BoxFuture;
use relay::Receiver as RelayReceiver;
use tokio_core::reactor::{Core, Timeout, Handle};
use tokio_core::net::{TcpListener, TcpStream};
use tokio_core::io::{Io, Framed};
use serde_json::{Value, to_value, from_value};

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

/// Preprocess the tripple returned by .start
///
/// So the error is checked that it didn't happen.
fn process_start(params: (Client, ServerCtl, RelayReceiver<Option<IoError>>)) -> (Client, ServerCtl, Box<Future<Item = (), Error = IoError>>) {
    let (client, ctl, finished) = params;
    let receiver = finished
        .map(|maybe_error| {
            assert!(maybe_error.is_none());
        })
        .map_err(|_| panic!("Canceled"));
    (client, ctl, Box::new(receiver))
}

/// Single RPC call
///
/// Run both the server and client, send a request and wait for the answer. The server terminates
/// after the first request, so we can wait for both to terminate.
#[test]
fn rpc_answer() {
    let (mut reactor, s1, s2) = prepare();
    let all = {
        // Run in a sub-block, so we drop all the clients, etc.
        let handle = reactor.handle();
        let (_client, _ctl, server_finished) = process_start(Endpoint::new(s1, AnswerServer).start(&handle));
        let (client, _ctl, client_endpoint_finished) = process_start(Endpoint::client_only(s2).start(&handle));
        client.call("test".to_owned(), None, None)
            .and_then(|(_client, answered)| answered)
            .map(|response| assert_eq!(json!(42), response.unwrap().result.unwrap()))
            .join3(server_finished, client_endpoint_finished)
    };
    reactor.run(all).unwrap();
}

/// Send a notification to the server.
#[test]
fn notification() {
    let (mut reactor, s1, s2) = prepare();
    let all = {
        // Run in a sub-block, so we drop all the clients, etc.
        let handle = reactor.handle();
        let (_client, _ctl, server_finished) = process_start(Endpoint::new(s1, AnswerServer).start(&handle));
        let (client, _ctl, client_endpoint_finished) = process_start(Endpoint::client_only(s2).start(&handle));
        client.notify("notif".to_owned(), None)
            .and_then(|_client| Ok(()))
            .join3(server_finished, client_endpoint_finished)
    };
    reactor.run(all).unwrap();
}

struct AnotherServer(Handle, Cell<usize>);

/// Another testing server
///
/// It answers the RPC "timeout" with waiting that as long as is provided in the first and second
/// argument (seconds and microseconds) and then sending true back. It rejects all other methods.
/// It terminates after receiving .1 requests.
impl Server for AnotherServer {
    type Success = bool;
    type RPCCallResult = BoxFuture<bool, ServerError>;
    type NotificationResult = Result<(), ()>;
    fn rpc(&self, ctl: &ServerCtl, method: &str, params: &Option<Value>) -> Option<Self::RPCCallResult> {
        let mut num = self.1.get();
        num -= 1;
        self.1.set(num);
        if num == 0 {
            ctl.terminate();
        }
        if method == "timeout" {
            let params: Vec<u64> = from_value(params.as_ref().unwrap().clone()).unwrap();
            let timeout = Timeout::new(Duration::new(params[0], params[1] as u32), &self.0)
                .unwrap()
                .map(|_| true)
                .map_err(|e| (-32000, "Server error".to_owned(), Some(to_value(format!("{}", e)).unwrap())))
                .boxed();
            Some(timeout)
        } else {
            None
        }
    }
}

/// Test where we call a non-existant method
///
/// And we get a proper error.
#[test]
fn wrong_method() {
    let (mut reactor, s1, s2) = prepare();
    let all = {
        // Run in a sub-block, so we drop all the clients, etc.
        let handle = reactor.handle();
        let (_client, _ctl, server_finished) = process_start(Endpoint::new(s1, AnotherServer(handle.clone(), Cell::new(1))).start(&handle));
        let (client, _ctl, client_endpoint_finished) = process_start(Endpoint::client_only(s2).start(&handle));
        client.call("wrong".to_owned(), None, None)
            .and_then(|(_client, answered)| answered)
            .map(|response| {
                assert_eq!(RPCError {
                               code: -32601,
                               message: "Method not found".to_owned(),
                               data: Some(json!("wrong")),
                           },
                           response.unwrap().result.unwrap_err());
            })
            .join3(server_finished, client_endpoint_finished)
    };
    reactor.run(all).unwrap();
}

/// Test we can get a timeout if the method takes a long time.
#[test]
fn timeout() {
    let (mut reactor, s1, s2) = prepare();
    let all = {
        // Run in a sub-block, so we drop all the clients, etc.
        let handle = reactor.handle();
        let (_client, _ctl, server_finished) = process_start(Endpoint::new(s1, AnotherServer(handle.clone(), Cell::new(1))).start(&handle));
        let (client, _ctl, client_endpoint_finished) = process_start(Endpoint::client_only(s2).start(&handle));
        client.call("timeout".to_owned(), Some(json!([3, 0])), Some(Duration::new(1, 0)))
            .and_then(|(_client, answered)| answered)
            .map(|response| assert!(response.is_none()))
            .join3(server_finished, client_endpoint_finished)
    };
    reactor.run(all).unwrap();
}

/// Test the server works even when there are some methods taking some time
#[test]
fn delayed() {
    let (mut reactor, s1, s2) = prepare();
    let all = {
        // Run in a sub-block, so we drop all the clients, etc.
        let handle = reactor.handle();
        let (_client, _ctl, server_finished) = process_start(Endpoint::new(s1, AnotherServer(handle.clone(), Cell::new(1))).start(&handle));
        let (client, _ctl, client_endpoint_finished) = process_start(Endpoint::client_only(s2).start(&handle));
        client.call("timeout".to_owned(), Some(json!([0, 500000000])), Some(Duration::new(1, 0)))
            .and_then(|(_client, answered)| answered)
            .map(|response| assert!(response.unwrap().result.unwrap().as_bool().unwrap()))
            .join3(server_finished, client_endpoint_finished)
    };
    reactor.run(all).unwrap();
}

/// Don't stop the server, wait only for the client
///
/// We check that the server works even if the finish future isn't waited for.
#[test]
fn client_only() {
    let (mut reactor, s1, s2) = prepare();
    let client = {
        let handle = reactor.handle();
        Endpoint::new(s1, AnotherServer(handle.clone(), Cell::new(2))).start(&handle);
        let (client, _ctl, client_endpoint_finished) = process_start(Endpoint::client_only(s2).start(&handle));
        client.call("timeout".to_owned(), Some(json!([0, 500000000])), None)
            .and_then(|(_client, answered)| answered)
            .map(move |response| {
                response.as_ref().unwrap();
                assert!(response.unwrap().result.unwrap().as_bool().unwrap());
            })
            .join(client_endpoint_finished)
    };
    reactor.run(client).unwrap();
}

/// Run two RPCs in parallel and see one can overtake the other
#[test]
fn parallel() {
    let (mut reactor, s1, s2) = prepare();
    let all = {
        // Run in a sub-block, so we drop all the clients, etc.
        let handle = reactor.handle();
        let (_client, _ctl, server_finished) = process_start(Endpoint::new(s1, AnotherServer(handle.clone(), Cell::new(2))).parallel(2).start(&handle));
        let (client, _ctl, client_endpoint_finished) = process_start(Endpoint::client_only(s2).start(&handle));
        let first_finished = Rc::new(Cell::new(false));
        let first_finished_cloned = first_finished.clone();
        let client1_finished = client.clone()
            .call("timeout".to_owned(), Some(json!([0, 500000000])), None)
            .and_then(|(_client, answered)| answered)
            .map(move |response| {
                assert!(response.unwrap().result.unwrap().as_bool().unwrap());
                first_finished_cloned.set(true);
            });
        let client2_finished = client.call("wrong".to_owned(), None, None)
            .and_then(|(_client, answered)| answered)
            .map(move |response| {
                assert_eq!(RPCError {
                               code: -32601,
                               message: "Method not found".to_owned(),
                               data: Some(json!("wrong")),
                           },
                           response.unwrap().result.unwrap_err());
                assert!(!first_finished.get());
            });
        server_finished.join4(client1_finished, client2_finished, client_endpoint_finished)
    };
    reactor.run(all).unwrap();
}

/// Similar to `parallel`, but doesn't allow running the RPCs in parallel
///
/// Also, send the second request after the client is received back from the asynchronous call.
#[test]
fn seq() {
    let (mut reactor, s1, s2) = prepare();
    let all = {
        let handle = reactor.handle();
        let (_client, _ctl, server_finished) = process_start(Endpoint::new(s1, AnotherServer(handle.clone(), Cell::new(2))).start(&handle));
        let (client, _ctl, client_endpoint_finished) = process_start(Endpoint::client_only(s2).start(&handle));
        client.call("timeout".to_owned(), Some(json!([0, 500000000])), None)
            .and_then(|(client, answered)| {
                let first_finished = Rc::new(Cell::new(false));
                let first_finished_cloned = first_finished.clone();
                let client2_finished = client.call("wrong".to_owned(), None, None)
                    .and_then(|(_client, answered)| answered)
                    .map(move |response| {
                        assert_eq!(RPCError {
                                       code: -32601,
                                       message: "Method not found".to_owned(),
                                       data: Some(json!("wrong")),
                                       },
                                       response.unwrap().result.unwrap_err());
                        assert!(!first_finished.get());
                    });
                answered.map(move |response| {
                    assert!(response.unwrap().result.unwrap().as_bool().unwrap());
                    first_finished_cloned.set(true);
                })
                .join(client2_finished)
            })
            .join3(server_finished, client_endpoint_finished)
    };
    reactor.run(all).unwrap();
}
