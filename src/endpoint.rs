// Copyright 2017 tokio-jsonrpc Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

//! The endpoint of the JSON RPC connection
//!
//! This module helps building the endpoints of the connection. The endpoints act as both client
//! and server at the same time. If you want a client-only endpoint, use
//! [`EmptyServer`](struct.EmptyServer.html) as the server. If you want a server-only endpoint,
//! simply don't call any RPCs or notifications.

use message::{Message, Parsed, Response, Request, Notification};

use std::io::{Error as IoError, ErrorKind};
use std::collections::HashMap;
use std::rc::Rc;

use serde::Serialize;
use serde_json::{Value, to_value};
use futures::{Future, IntoFuture, Stream, Sink};
use futures::stream::{self, Once, empty};
use futures_mpsc::{channel, Sender};
use relay::{channel as relay_channel, Sender as RelaySender};
use tokio_core::reactor::Handle;

/// The server endpoint
///
/// This is usually implemented by the end application and provides the actual functionality of the
/// RPC server. It allows composition of more servers together.
///
/// In future it might be possible to generate servers with the help of some macros. Currently it
/// is up to the developer to handle conversion of parameters, etc.
///
/// The default implementations of the callbacks return None, indicating that the given method is
/// not known. It allows implementing only rpcs or only notifications without having to worry about
/// the other callback. If you want a server that knows nothing at all, use
/// [`EmptyServer`](struct.EmptyServer.html).
pub trait Server {
    /// The successfull result of the RPC call.
    type Success: Serialize;
    /// The result of the RPC call
    ///
    /// Once the future resolves, the value or error is sent to the client as the reply. The reply
    /// is wrapped automatically.
    type RPCCallResult: IntoFuture<Item = Self::Success, Error = (i64, String, Option<Value>)>;
    /// The result of the RPC call
    ///
    /// As the client doesn't expect anything in return, both the success and error results are
    /// thrown away and therefore (). However, it still makes sense to distinguish success and
    /// error.
    // TODO: Why do we need 'static here and not above?
    type NotificationResult: IntoFuture<Item = (), Error = ()> + 'static;
    /// Called when the client requests something
    ///
    /// This is a callback from the [endpoint](struct.Endpoint.html) when the client requests
    /// something. If the method is unknown, it shall return `None`. This allows composition of
    /// servers.
    ///
    /// Conversion of parameters and handling of errors is up to the implementer of this trait.
    fn rpc(&self, _method: &str, _params: &Option<Value>) -> Option<Self::RPCCallResult> {
        None
    }
    /// Called when the client sends a notification
    ///
    /// This is a callback from the [endpoint](struct.Endpoint.html) when the client requests
    /// something. If the method is unknown, it shall return `None`. This allows composition of
    /// servers.
    ///
    /// Conversion of parameters and handling of errors is up to the implementer of this trait.
    fn notification(&self, _method: &str, _params: &Option<Value>) -> Option<Self::NotificationResult> {
        None
    }
}

// Our own BoxFuture & friends that is *not* send. We don't do send.
type BoxFuture<T, E> = Box<Future<Item = T, Error = E>>;
type FutureMessage = BoxFuture<Option<Message>, IoError>;
type BoxStream<T, E> = Box<Stream<Item = T, Error = E>>;

// A future::stream::once that takes only the success value, for convenience.
fn once<T, E>(item: T) -> Once<T, E> {
    stream::once(Ok(item))
}

fn do_request<RPCServer: Server + 'static>(server: &RPCServer, request: Request) -> FutureMessage {
    match server.rpc(&request.method, &request.params) {
        None => Box::new(Ok(Some(Message::error(-32601, "Method not found".to_owned(), Some(Value::String(request.method.clone()))))).into_future()),
        Some(future) => {
            Box::new(future.into_future().then(move |result| match result {
                Err((code, msg, data)) => Ok(Some(request.error(code, msg, data))),
                Ok(result) => Ok(Some(request.reply(to_value(result).expect("Trying to return a value that can't be converted to JSON")))),
            }))
        },
    }
}

fn do_notification<RPCServer: Server>(server: &RPCServer, notification: Notification) -> FutureMessage {
    match server.notification(&notification.method, &notification.params) {
        None => Box::new(Ok(None).into_future()),
        Some(future) => Box::new(future.into_future().then(|_| Ok(None))),
    }
}

/// A RPC server that knows no methods
///
/// You can use this if you want to have a client-only [Endpoint](struct.Endpoint.html). It simply
/// refuses all the methods passed to it.
pub struct EmptyServer;

impl Server for EmptyServer {
    type Success = ();
    type RPCCallResult = Result<(), (i64, String, Option<Value>)>;
    type NotificationResult = Result<(), ()>;
}

pub struct Client {
    idmap: Rc<HashMap<String, RelaySender<Response>>>,
    sender: Sender<Message>,
}

fn msg_handle<RPCServer: Server + 'static>(server: &RPCServer, msg: Parsed) -> BoxStream<FutureMessage, IoError> {
    match msg {
        Err(broken) => {
            let err: FutureMessage = Ok(Some(broken.reply())).into_future().boxed();
            Box::new(once(err))
        },
        Ok(Message::Request(req)) => Box::new(once(do_request(server, req))),
        Ok(Message::Notification(notif)) => Box::new(once(do_notification(server, notif))),
        _ => Box::new(empty()),
    }
}

// TODO: Some other interface to this?
pub fn endpoint<Connection, RPCServer>(handle: Handle, connection: Connection, server: RPCServer) -> Client
    where Connection: Stream<Item = Parsed, Error = IoError> + Sink<SinkItem = Message, SinkError = IoError> + Send + 'static,
          RPCServer: Server + 'static
{
    let (sender, receiver) = channel(32);
    let idmap = Rc::new(HashMap::new());
    let client = Client {
        idmap: idmap.clone(),
        sender: sender,
    };
    let (sink, stream) = connection.split();
    // Create a future for each received item that'll return something. Run some of them in
    // parallel.

    // TODO: Have a concrete enum-type for the futures so we don't have to allocate and box it.
    let answers = stream.map(move |parsed| msg_handle(&server, parsed))
        .flatten()
        .buffer_unordered(4)
        .filter_map(|message| message);
    // Take both the client RPCs and the answers
    let outbound = answers.select(receiver.map_err(|_| IoError::new(ErrorKind::Other, "Shouldn't happen")));
    // And send them all
    let transmitted = sink.send_all(outbound);
    // Once the last thing is sent, we're done
    // TODO: Something with the errors
    handle.spawn(transmitted.map(|_| ()).map_err(|_| ()));
    client
}
