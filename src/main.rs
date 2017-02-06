// Copyright (c) 2017 Michal 'vorner' Vaner <vorner@vorner.cz>
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

extern crate tokio_jsonrpc;

extern crate futures;
extern crate tokio_core;
extern crate serde_json;

use tokio_jsonrpc::{Message, LineCodec};

use std::io::Error;

use futures::{Future, Sink, Stream};
use futures::stream::{once, Once};
use tokio_core::reactor::Core;
use tokio_core::net::TcpListener;
use tokio_core::io::Io;
use serde_json::Value;

fn main() {
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let listener = TcpListener::bind(&"127.0.0.1:2345".parse().unwrap(), &handle).unwrap();
    let connections = listener.incoming();
    let service = connections.for_each(|(stream, _)| {
        let jsonized = stream.framed(LineCodec);
        let (w, r) = jsonized.split();
        let header: Once<_, Error> = once(Ok(Message::Batch(vec![Message::request("Hello".to_owned(), Some(Value::Null)),
                                                                 Message::Unmatched(Value::Null).error(42, "Wrong!".to_owned(), None),
                                                                 Message::notification("Alert!".to_owned(), Some(Value::String("blabla".to_owned()))),
                                                                 Message::Unmatched(Value::String(String::new()))])));
        let sent = w.send_all(header)
            .and_then(|(sink, _)| sink.send_all(r))
            .map(|_| ())
            .map_err(|_| {
                // TODO Something with the error ‒ logging?
                ()
            });
        // Do the sending in the background
        handle.spawn(sent);
        Ok(())
    });
    core.run(service).unwrap();
}
