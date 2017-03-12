# 0.4.0:

* Rename RPCError to RpcError (and similar), according to the style guide.
* Move the `Server` trait to its own module.
* Provide some tools to compose servers (`AbstractServer`, `ServerChain`).

# 0.3.0:

Cleanups in documentaion in API. The functionality is the same, but the API is
a bit different. Bumping version because of that incompatibility.

# 0.2.0:

Added the endpoint infrastructure. It allows calling and handling the RPCs and
notifications.
