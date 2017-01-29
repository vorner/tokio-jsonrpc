extern crate futures;
extern crate tokio_core;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

use std::io::{Result as IoResult, Error, ErrorKind};

use futures::{Future, Sink, Stream};
use futures::stream::{once, Once};
use tokio_core::reactor::Core;
use tokio_core::net::TcpListener;
use tokio_core::io::{Codec,EasyBuf,Io};
use serde::ser::{Serialize, Serializer, SerializeStruct};
use serde_json::Value;
use serde_json::ser::to_vec;
use serde_json::de::from_slice;

// TODO: Modify the serialize/deserialize so it generates the right JSON

#[derive(Debug, Deserialize)]
struct Version;

impl Serialize for Version {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str("2.0")
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Request {
    jsonrpc: Version,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    pub id: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RPCError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct Response {
    pub result: Result<Value, RPCError>,
    pub id: Value,
}

impl Serialize for Response {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sub = serializer.serialize_struct("Response", 2)?;
        sub.serialize_field("id", &self.id)?;
        match self.result {
            Ok(ref value) => sub.serialize_field("result", value),
            Err(ref err) => sub.serialize_field("error", err),
        }?;
        sub.end()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Notification {
    jsonrpc: Version,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub enum Message {
    Request(Request),
    Response(Response),
    Notification(Notification),
    Batch(Vec<Message>),
    Unmatched(Value),
}

impl Serialize for Message {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match *self {
            Message::Request(ref req) => req.serialize(serializer),
            Message::Response(ref resp) => resp.serialize(serializer),
            Message::Notification(ref notif) => notif.serialize(serializer),
            Message::Batch(ref batch) => batch.serialize(serializer),
            Message::Unmatched(ref val) => val.serialize(serializer),
        }
    }
}

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
        let header: Once<_, Error> = once(Ok(Message::Batch(
            vec![
                Message::Request(Request { jsonrpc: Version, method: "Hello".to_owned(), params: Some(Value::Null), id: Value::Bool(true) }),
                Message::Response(Response { result: Ok(Value::Bool(false)), id: Value::Bool(true) }),
                Message::Response(Response { result: Err(RPCError { code: 42, message: "Wrong!".to_owned(), data: None }), id: Value::Null }),
                Message::Notification(Notification { jsonrpc: Version, method: "Alert!".to_owned(), params: Some(Value::String("blabla".to_owned())) }),
                Message::Unmatched(Value::String(String::new())),
            ])));
        let sent = w.send_all(header).and_then(|(sink, _)| sink.send_all(r))
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
