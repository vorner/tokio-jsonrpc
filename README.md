# Tokio-JSONRPC

[![Travis Build Status](https://api.travis-ci.org/vorner/tokio-jsonrpc.png?branch=master)](https://travis-ci.org/vorner/tokio-jsonrpc)
[![AppVeyor Build status](https://ci.appveyor.com/api/projects/status/ygytb97bion810ru/branch/master?svg=true)](https://ci.appveyor.com/project/vorner/tokio-jsonrpc/branch/master)

This is an implementation of the [JSON RPC
2.0](http://www.jsonrpc.org/specification) protocol for tokio. It can handle
some of the more niche features, like batches and an endpoint being both the
server and the client at the same time.

This is *work in progress*, functionality might still be missing and the API of
what exists is likely to change in small ways. However, it probably can be used
for real work, if you don't mind having to update your code in the future.

Currently it contains the lower-level parts, parsing the messages and sending
anwers. A small example how to use these can be found in the [echo
params](examples/echo_params.rs) program.

Also, a bit of higher level is there in the form of the `Endpoint` structure
and some helpers in the `server` module. The use can be examined in the [time
server](examples/time_server.rs) program.

The API documentation can be found [here](https://docs.rs/tokio-jsonrpc).

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms
or conditions.
