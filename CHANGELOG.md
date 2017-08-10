* `ServerCtl::client()` returns a result instead of panicking if the connection
  was already terminated. It turns out if the `ServerCtl` is cloned and kept,
  it may have panicked due to external influences.

# 0.8.0

* Bump version of serde to 1.0. This brings the need to bump the version to
  downstream crates and it changes its API.

# 0.7.3

* Extend the `jsonrpc_params!` macro to allow returning errors instead of early
  exit in `named` and `positional` modes in addition to the auto-detected one.
* Cleanups.
* Another example added.

# 0.7.2

* Extention of the `jsonrpc_params!` macro to allow returning errors instead of early exit.

# 0.7.1

* Fix termination of the endpoint when the other end drops connection.

# 0.7.0

* Modified the `jsonrpc_params` macro:
  - Several cases don't take names, when not needed (eg. single-value decoding
    or positional decoding).
  - The names are now taken as an expression, allowing names legal in JSON
    strings, but illegal as rust identifiers. They are not used as identifiers
    anyway.

# 0.6.0

* Fixes for newer slog (it did an API incompatible change with a patch version
  bump).
* Faster compilation times in test/debug builds (by splitting long chains of
  future/stream modifiers with trait objects).
* Adapting to tokio's new traits. This is a breaking change in our API, but
  changing import of `tokio_core::io::Io` to `tokio_io::AsyncRead` is usually
  enough.

# 0.5.1

* Logging support (the endpoint now can be fed with a logger).

# 0.5.0

* A macro `jsonrpc_params` to conveniently read and convert RPC call or
  notification parameters to expected ones is provided. It also handles errors
  about invalid parameters.
* `RpcError::invalid_params` now takes optional error message.

# 0.4.0:

* Rename `RPCError` to `RpcError` (and similar), according to the style guide.
* Move the `Server` trait to its own module.
* Provide some tools to compose servers (`AbstractServer`, `ServerChain`).

# 0.3.0:

Cleanups in documentaion in API. The functionality is the same, but the API is
a bit different. Bumping version because of that incompatibility.

# 0.2.0:

Added the endpoint infrastructure. It allows calling and handling the RPCs and
notifications.
