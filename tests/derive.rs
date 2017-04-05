// Copyright 2017 tokio-jsonrpc Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

//! A server that responds with the current time
//!
//! A server listening on localhost:2345. It reponds to the „now“ method, returning the current
//! unix timestamp (number of seconds since 1.1. 1970). You can also subscribe to periodic time
//! updates.

#[macro_use]
extern crate tokio_jsonrpc_derive;
#[macro_use]
extern crate tokio_jsonrpc;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;

use tokio_jsonrpc::RpcError;

#[derive(Debug, Deserialize, Params, PartialEq)]
struct Subscribe {
    secs: u64,
    #[serde(default)]
    nsecs: u32,
}

#[test]
fn the_test() {
    fn _inner() -> Option<Result<(), RpcError>> {
        let params = params!({"secs": 76});
        let parsed: Subscribe = parse_params!(params);
        assert_eq!(parsed, Subscribe { secs: 76, nsecs: 0 });
        Some(Ok(()))
    }

    _inner().unwrap().unwrap();

    let params = params!({"secs": 8, "nsecs": 2});
    let parsed: Subscribe = try_parse_params!(params).unwrap();
    assert_eq!(parsed, Subscribe { secs: 8, nsecs: 2 });
}
