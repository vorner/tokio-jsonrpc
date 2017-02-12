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

use message::RPCError;

use serde::Serialize;
use serde_json::Value;
use futures::IntoFuture;

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
    type RPCCallResult: IntoFuture<Item = Self::Success, Error = RPCError>;
    /// The result of the RPC call
    ///
    /// As the client doesn't expect anything in return, both the success and error results are
    /// thrown away and therefore (). However, it still makes sense to distinguish success and
    /// error.
    type NotificationResult: IntoFuture<Item = (), Error = ()>;
    /// Called when the client requests something
    ///
    /// This is a callback from the [endpoint](struct.Endpoint.html) when the client requests
    /// something. If the method is unknown, it shall return `None`. This allows composition of
    /// servers.
    ///
    /// Conversion of parameters and handling of errors is up to the implementer of this trait.
    fn rpc(_method: &str, _params: &Option<Value>) -> Option<Self::RPCCallResult> {
        None
    }
    /// Called when the client sends a notification
    ///
    /// This is a callback from the [endpoint](struct.Endpoint.html) when the client requests
    /// something. If the method is unknown, it shall return `None`. This allows composition of
    /// servers.
    ///
    /// Conversion of parameters and handling of errors is up to the implementer of this trait.
    fn notification(_method: &str, _params: &Option<Value>) -> Option<Self::NotificationResult> {
        None
    }
}

/// A RPC server that knows no methods
///
/// You can use this if you want to have a client-only [Endpoint](struct.Endpoint.html). It simply
/// refuses all the methods passed to it.
pub struct EmptyServer;

impl Server for EmptyServer {
    type Success = ();
    type RPCCallResult = Result<(), RPCError>;
    type NotificationResult = Result<(), ()>;
}
