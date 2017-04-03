// Copyright 2017 tokio-jsonrpc Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

//! A JSON RPC protocol for the [tokio](https://tokio.rs) framework.
//!
//! This implements the handling of the
//! [JSON RPC 2.0](http://www.jsonrpc.org/specification) specification. The low-level parts are in
//! the [`message`](message/index.html) and the [`codec`](codec/index.html) modules. The first
//! draft of the higher-lever API is in the [`endpoint`](endpoint/index.html) module. Some helpers
//! to compose the server part is in the [`server`](server/index.html) module.
//!
//! # Examples
//!
//! A skeleton of reading messages from the other side, mapping them to answers and sending them
//! back.
//!
//! ```rust
//! # extern crate tokio_core;
//! # extern crate tokio_io;
//! # extern crate tokio_jsonrpc;
//! # extern crate futures;
//! #
//! # use tokio_core::reactor::Core;
//! # use tokio_core::net::TcpListener;
//! # use tokio_io::AsyncRead;
//! # use tokio_jsonrpc::LineCodec;
//! # use futures::{Stream, Sink, Future};
//! #
//! # fn main() {
//! let mut core = Core::new().unwrap();
//! let handle = core.handle();
//!
//! let listener = TcpListener::bind(&"127.0.0.1:2345".parse().unwrap(), &handle).unwrap();
//! let connections = listener.incoming();
//! let service = connections.for_each(|(stream, _)| {
//!     let messages = stream.framed(LineCodec::new());
//!     let (write, read) = messages.split();
//!     let answers = read.filter_map(|message| {
//!         match message {
//!             _ => unimplemented!(),
//!         }
//!     });
//!     handle.spawn(write.send_all(answers).map(|_| ()).map_err(|_| ()));
//!     Ok(())
//! });
//! # }
//! ```
//!
//! Provide a server that greets through an RPC.
//!
//! ```rust,no_run
//! # extern crate tokio_core;
//! # extern crate tokio_io;
//! # extern crate tokio_jsonrpc;
//! # extern crate futures;
//! # extern crate serde_json;
//! #
//! # use tokio_core::reactor::Core;
//! # use tokio_core::net::TcpListener;
//! # use tokio_io::AsyncRead;
//! # use tokio_jsonrpc::{LineCodec, Server, ServerCtl, Params, RpcError, Endpoint};
//! # use futures::{Future, Stream};
//! # use serde_json::Value;
//! #
//! # fn main() {
//! let mut core = Core::new().unwrap();
//! let handle = core.handle();
//!
//! let listener = TcpListener::bind(&"127.0.0.1:2346".parse().unwrap(), &handle).unwrap();
//!
//! struct UselessServer;
//!
//! impl Server for UselessServer {
//!     type Success = String;
//!     type RpcCallResult = Result<String, RpcError>;
//!     type NotificationResult = Result<(), ()>;
//!     fn rpc(&self,
//!            ctl: &ServerCtl,
//!            method: &str,
//!            _params: &Option<Params>)
//!         -> Option<Self::RpcCallResult> {
//!         match method {
//!             // Accept a hello message and finish the greeting
//!             "hello" => Some(Ok("world".to_owned())),
//!             // When the other side says bye, terminate the connection
//!             "bye" => {
//!                 ctl.terminate();
//!                 Some(Ok("bye".to_owned()))
//!             },
//!             _ => None
//!         }
//!     }
//! }
//!
//! let connections = listener.incoming().for_each(|(stream, _)| {
//!     // Greet every new connection
//!     let (client, _) = Endpoint::new(stream.framed(LineCodec::new()), UselessServer)
//!         .start(&handle);
//!     let notified = client.notify("hello".to_owned(), None)
//!         .map(|_| ())
//!         .map_err(|_| ());
//!     handle.spawn(notified);
//!     Ok(())
//! });
//!
//! core.run(connections).unwrap();
//! # }
//! ```

extern crate serde;
// We use the json! macro only in the tests
#[cfg_attr(test, macro_use)]
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate uuid;
extern crate bytes;
extern crate tokio_core;
extern crate tokio_io;
extern crate futures;
#[macro_use]
extern crate slog;

pub mod codec;
pub mod endpoint;
#[macro_use]
pub mod message;
pub mod server;

/// This contains some reexports so macros can find them.
///
/// It isn't for the direct use of the library consumer.
pub mod macro_exports {
    pub use serde_json::{Value, Map, from_value, to_value};
    pub use std::option::Option;
    pub use std::result::Result;
}

pub use codec::{Boundary as BoundaryCodec, Line as LineCodec};
pub use endpoint::{Client, Endpoint, ServerCtl};
pub use message::{Message, Params, Parsed, RpcError};
pub use server::Server;
