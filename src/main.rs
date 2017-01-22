extern crate futures;
extern crate tokio_core;
extern crate serde_json;

use std::io::{Result, Error, ErrorKind};

use futures::{Future, Sink, Stream};
use tokio_core::reactor::Core;
use tokio_core::net::TcpListener;
use tokio_core::io::{Codec,EasyBuf,Io};
use serde_json::Value;
use serde_json::ser::to_vec;
use serde_json::de::from_slice;

struct JsonCodec;

impl Codec for JsonCodec {
    type In = Value;
    type Out = Value;
    fn decode(&mut self, buf: &mut EasyBuf) -> Result<Option<Value>> {
        // TODO: Optimise by storing the previous position if we didn't find it
        if let Some(i) = buf.as_slice().iter().position(|&b| b == b'\n') {
            let line = buf.drain_to(i);
            buf.drain_to(1);
            from_slice(line.as_slice())
                .map(|val| Some(val))
                .map_err(|e| Error::new(ErrorKind::Other, e))
        } else {
            Ok(None)
        }
    }
    fn encode(&mut self, msg: Value, buf: &mut Vec<u8>) -> Result<()> {
        *buf = to_vec(&msg).map_err(|e| Error::new(ErrorKind::Other, e))?;
        Ok(())
    }
}

fn main() {
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let listener = TcpListener::bind(&"127.0.0.1:2345".parse().unwrap(), &handle).unwrap();
    let connections = listener.incoming();
    let service = connections.for_each(|(stream, _)| {
        let parsed = stream.framed(JsonCodec);
        let (w, r) = parsed.split();
        let sent = w.send_all(r)
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
