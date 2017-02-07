// Copyright 2017 tokio-jsonrpc Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
// Copyright (c) 2017 Michal 'vorner' Vaner <vorner@vorner.cz>

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

pub use message::Message;
pub use codec::Codec;
