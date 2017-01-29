use serde::ser::{Serialize, Serializer, SerializeStruct};
use serde_json::Value;

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
    // TODO: Make private?
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

impl Message {
    pub fn request(method: String, params: Option<Value>) -> Self {
        Message::Request(Request {
            jsonrpc: Version,
            method: method,
            params: params,
            // TODO!
            id: Value::Null,
        })
    }
    pub fn notification(method: String, params: Option<Value>) -> Self {
        Message::Notification(Notification {
            jsonrpc: Version,
            method: method,
            params: params,
        })
    }
    // TODO: Other constructors
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
