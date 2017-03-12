// Copyright 2017 tokio-jsonrpc Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

//! The [`Server`](trait.Server.html) trait and helpers.
//!
//! The `Server` trait for the use by the [`Endpoint`](../endpoint/struct.Endpoint.html) is defined
//! here. Furthermore, some helpers for convenient creation and composition of servers are
//! available. Note that not all of these helpers are necessarily zero-cost, at least at this time.

use futures::IntoFuture;
use serde::Serialize;
use serde_json::Value;

use endpoint::ServerCtl;
use message::RPCError;

/// The server endpoint.
///
/// This is usually implemented by the end application and provides the actual functionality of the
/// RPC server. It allows composition of more servers together.
///
/// The default implementations of the callbacks return None, indicating that the given method is
/// not known. It allows implementing only RPCs or only notifications without having to worry about
/// the other callback. If you want a server that does nothing at all, use
/// [`Empty`](struct.Empty.html).
pub trait Server {
    /// The successfull result of the RPC call.
    type Success: Serialize;
    /// The result of the RPC call
    ///
    /// Once the future resolves, the value or error is sent to the client as the reply. The reply
    /// is wrapped automatically.
    type RPCCallResult: IntoFuture<Item = Self::Success, Error = RPCError>;
    /// The result of the RPC call.
    ///
    /// As the client doesn't expect anything in return, both the success and error results are
    /// thrown away and therefore (). However, it still makes sense to distinguish success and
    /// error.
    type NotificationResult: IntoFuture<Item = (), Error = ()> + 'static;
    /// Called when the client requests something.
    ///
    /// This is a callback from the [endpoint](struct.Endpoint.html) when the client requests
    /// something. If the method is unknown, it shall return `None`. This allows composition of
    /// servers.
    ///
    /// Conversion of parameters and handling of errors is up to the implementer of this trait.
    fn rpc(&self, _ctl: &ServerCtl, _method: &str, _params: &Option<Value>) -> Option<Self::RPCCallResult> {
        None
    }
    /// Called when the client sends a notification.
    ///
    /// This is a callback from the [endpoint](struct.Endpoint.html) when the client requests
    /// something. If the method is unknown, it shall return `None`. This allows composition of
    /// servers.
    ///
    /// Conversion of parameters and handling of errors is up to the implementer of this trait.
    fn notification(&self, _ctl: &ServerCtl, _method: &str, _params: &Option<Value>) -> Option<Self::NotificationResult> {
        None
    }
    /// Called when the endpoint is initialized.
    ///
    /// It provides a default empty implementation, which can be overriden to hook onto the
    /// initialization.
    fn initialized(&self, _ctl: &ServerCtl) {}
}

/// A RPC server that knows no methods.
///
/// You can use this if you want to have a client-only [Endpoint](struct.Endpoint.html). It simply
/// terminates the server part right away. Or, more conveniently, use `Endpoint`'s
/// [`client_only`](struct.Endpoint.html#method.client_only) method.
pub struct Empty;

impl Server for Empty{
    type Success = ();
    type RPCCallResult = Result<(), RPCError>;
    type NotificationResult = Result<(), ()>;
    fn initialized(&self, ctl: &ServerCtl) {
        ctl.terminate();
    }
}
