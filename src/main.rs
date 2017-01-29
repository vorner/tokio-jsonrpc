extern crate futures;
extern crate tokio_core;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

use std::io::{Result, Error, ErrorKind};

use futures::{Future, Sink, Stream, Poll, StartSend, AsyncSink, Async};
use tokio_core::reactor::Core;
use tokio_core::net::TcpListener;
use tokio_core::io::{Codec,EasyBuf,Io};
use serde_json::Value;
use serde_json::ser::to_vec;
use serde_json::de::from_slice;
use serde_json::value::{to_value, from_value};

// TODO: Modify the serialize/deserialize so it generates the right JSON

#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
    pub method: String,
    pub params: Option<Value>,
    pub id: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RPCError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub result: Option<Value>,
    pub error: Option<RPCError>,
    pub id: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Notification {
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Message {
    Request(Request),
    Response(Response),
    Notification(Notification),
    Batch(Vec<Message>),
    Unmatched(Value),
}

fn err_map(e: serde_json::error::Error) -> Error {
    Error::new(ErrorKind::Other, e)
}

struct JsonCodec;

impl Codec for JsonCodec {
    type In = Value;
    type Out = Value;
    fn decode(&mut self, buf: &mut EasyBuf) -> Result<Option<Value>> {
        // TODO: Use object boundary instead of newlines
        if let Some(i) = buf.as_slice().iter().position(|&b| b == b'\n') {
            let line = buf.drain_to(i);
            buf.drain_to(1);
            from_slice(line.as_slice()).map(Some).map_err(err_map)
        } else {
            Ok(None)
        }
    }
    fn encode(&mut self, msg: Value, buf: &mut Vec<u8>) -> Result<()> {
        *buf = to_vec(&msg).map_err(err_map)?;
        buf.push(b'\n');
        Ok(())
    }
}

pub struct RPCFramed<JsonStream>(JsonStream);

impl<JsonStream> RPCFramed<JsonStream> where
    JsonStream: Stream<Item = Value, Error = Error> + Sink<SinkItem = Value, SinkError = Error>
{
    pub fn new(stream: JsonStream) -> Self {
        RPCFramed(stream)
    }
}

impl<JsonStream> Sink for RPCFramed<JsonStream> where
    JsonStream: Stream<Item = Value, Error = Error> + Sink<SinkItem = Value, SinkError = Error>
{
    type SinkError = Error;
    type SinkItem = Message;
    fn start_send(&mut self, item: Message) -> StartSend<Message, Error> {
        let converted = to_value(&item).map_err(err_map)?;
        match self.0.start_send(converted) {
            Ok(AsyncSink::NotReady(_)) => Ok(AsyncSink::NotReady(item)),
            other => other.map(|_| AsyncSink::Ready),
        }
    }
    fn poll_complete(&mut self) -> Poll<(), Error> {
        self.0.poll_complete()
    }
}

// TODO: Implement some kind of error handling other than just bailing out?
// But that is going to be tricky with the batch where some of the items might error and some not.
impl<JsonStream> Stream for RPCFramed<JsonStream> where
    JsonStream: Stream<Item = Value, Error = Error> + Sink<SinkItem = Value, SinkError = Error>
{
    type Error = Error;
    type Item = Message;
    fn poll(&mut self) -> Poll<Option<Message>, Error> {
        match self.0.poll() {
            Err(e) => Err(e),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Ok(Async::Ready(None)) => Ok(Async::Ready(None)),
            Ok(Async::Ready(Some(value))) => {
                Ok(Async::Ready(Some(from_value(value).map_err(err_map)?)))
            },
        }
    }
}

fn main() {
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let listener = TcpListener::bind(&"127.0.0.1:2345".parse().unwrap(), &handle).unwrap();
    let connections = listener.incoming();
    let service = connections.for_each(|(stream, _)| {
        let jsonized = stream.framed(JsonCodec);
        let rpcized = RPCFramed::new(jsonized);
        let (w, r) = rpcized.split();
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
