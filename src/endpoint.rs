// Copyright 2017 tokio-jsonrpc Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

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
pub trait Server {
    /// The successfull result of RPC call.
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
    fn rpc(method: &str, params: &Option<Value>) -> Option<Self::RPCCallResult>;
    /// Called when the client sends a notification
    ///
    /// This is a callback from the [endpoint](struct.Endpoint.html) when the client requests
    /// something. If the method is unknown, it shall return `None`. This allows composition of
    /// servers.
    ///
    /// Conversion of parameters and handling of errors is up to the implementer of this trait.
    fn notification(method: &str, params: &Option<Value>) -> Option<Self::NotificationResult>;
}
