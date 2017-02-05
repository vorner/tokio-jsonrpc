# Tokio-JSONRPC

[![Travis Build Status](https://api.travis-ci.org/vorner/tokio-jsonrpc.png?branch=master)](https://travis-ci.org/vorner/tokio-jsonrpc)
[![AppVeyor Build status](https://ci.appveyor.com/api/projects/status/ygytb97bion810ru/branch/master?svg=true)](https://ci.appveyor.com/project/vorner/tokio-jsonrpc/branch/master)

This is an implementation of the [JSON RPC
2.0](http://www.jsonrpc.org/specification) protocol for tokio. It can handle
some of the more niche features, like batches and an endpoint being both the
server and the client at the same time.

This is *work in progress*, most of the functionality is still missing and the
API of what exists is likely to change.


