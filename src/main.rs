extern crate tokio_jsonrpc;

extern crate futures;
extern crate tokio_core;
extern crate serde_json;

use tokio_jsonrpc::message::Message;

use std::io::{Result as IoResult, Error, ErrorKind};

use futures::{Future, Sink, Stream};
use futures::stream::{once, Once};
use tokio_core::reactor::Core;
use tokio_core::net::TcpListener;
use tokio_core::io::{Codec, EasyBuf, Io};
use serde_json::Value;
use serde_json::de::from_slice;
use serde_json::ser::to_vec;

fn err_map(e: serde_json::error::Error) -> Error {
    Error::new(ErrorKind::Other, e)
}

struct RPCCodec;

impl Codec for RPCCodec {
    type In = Message;
    type Out = Message;
    fn decode(&mut self, buf: &mut EasyBuf) -> IoResult<Option<Message>> {
        // TODO: Use object boundary instead of newlines
        if let Some(i) = buf.as_slice().iter().position(|&b| b == b'\n') {
            let line = buf.drain_to(i);
            buf.drain_to(1);
            from_slice(line.as_slice()).map(Some).map_err(err_map)
        } else {
            Ok(None)
        }
    }
    fn encode(&mut self, msg: Message, buf: &mut Vec<u8>) -> IoResult<()> {
        *buf = to_vec(&msg).map_err(err_map)?;
        buf.push(b'\n');
        Ok(())
    }
}

fn main() {
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let listener = TcpListener::bind(&"127.0.0.1:2345".parse().unwrap(), &handle).unwrap();
    let connections = listener.incoming();
    let service = connections.for_each(|(stream, _)| {
        let jsonized = stream.framed(RPCCodec);
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
