// Copyright 2017 tokio-jsonrpc Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

//! A JSON RPC protocol for the [tokio](https://tokio.rs) framework.
//!
//! This implements the handling of the
//! [JSON RPC 2.0](http://www.jsonrpc.org/specification) specification. Currently, only the
//! low-level parts are implemented, more will come in future versions.
//!
//! # Examples
//!
//! ```rust
//! # extern crate tokio_core;
//! # extern crate tokio_jsonrpc;
//! # extern crate futures;
//! #
//! # use tokio_core::reactor::Core;
//! # use tokio_core::net::TcpListener;
//! # use tokio_core::io::Io;
//! # use tokio_jsonrpc::LineCodec;
//! # use futures::{Stream, Sink, Future};
//! #
//! # fn main() {
//! # let mut core = Core::new().unwrap();
//! # let handle = core.handle();
//! #
//! # let listener = TcpListener::bind(&"127.0.0.1:2345".parse().unwrap(), &handle).unwrap();
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

extern crate serde;
// We use the json! macro only in the tests
#[cfg_attr(test, macro_use)]
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate uuid;
extern crate tokio_core;

pub mod message;
pub mod codec;

pub use message::{Message, Parsed};
pub use codec::{Boundary as BoundaryCodec, Line as LineCodec};
