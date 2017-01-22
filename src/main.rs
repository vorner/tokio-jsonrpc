extern crate futures;
extern crate tokio_core;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

use std::io::{Result, Error, ErrorKind};

use futures::{Future, Sink, Stream};
use tokio_core::reactor::Core;
use tokio_core::net::TcpListener;
use tokio_core::io::{Codec,EasyBuf,Io};
use serde_json::Value;
use serde_json::ser::to_vec;
use serde_json::de::from_slice;

// TODO: This isn't exactly what we want. We want to have an enum (request, result, notification) ‒
// can serde parse that?
#[derive(Debug, Deserialize)]
struct Incoming {
    // Should be 2.0
    jsonrpc: String,
    method: String,
    params: Option<Value>,
    id: Option<Value>,
}

#[derive(Debug, Serialize)]
struct Outgoing {
    result: Value,
    error: Value,
    id: Value
}

struct JsonCodec;

impl Codec for JsonCodec {
    type In = Incoming;
    type Out = Outgoing;
    fn decode(&mut self, buf: &mut EasyBuf) -> Result<Option<Incoming>> {
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
    fn encode(&mut self, msg: Outgoing, buf: &mut Vec<u8>) -> Result<()> {
        *buf = to_vec(&msg).map_err(|e| Error::new(ErrorKind::Other, e))?;
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
        let parsed = stream.framed(JsonCodec);
        let (w, r) = parsed.split();
        let transferred = r.filter_map(|input|
            match input.id {
                Some(id) => Some(Outgoing {
                    result: input.params.unwrap_or(Value::Null),
                    error: Value::Null,
                    id: id
                }),
                None => None,
            });
        let sent = w.send_all(transferred)
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
