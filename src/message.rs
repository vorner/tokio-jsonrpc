//! A JSON-RPC 2.0 messages
//!
//! The main entrypoint here is the [Message](enum.Message.html). The others are just building
//! blocks and you should generally work with `Message` instead.

use std::str::FromStr;

use serde::ser::{Serialize, Serializer, SerializeStruct};
use serde::de::{Deserialize, Deserializer, Unexpected, Error};
use serde_json::{Value, from_value};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Version;

impl Serialize for Version {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str("2.0")
    }
}

impl Deserialize for Version {
    fn deserialize<D: Deserializer>(deserializer: D) -> Result<Self, D::Error> {
        // The version is actually a string
        let parsed: String = Deserialize::deserialize(deserializer)?;
        if parsed == "2.0" {
            Ok(Version)
        } else {
            Err(D::Error::invalid_value(Unexpected::Str(&parsed), &"value 2.0"))
        }
    }
}

/// An RPC request
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Request {
    jsonrpc: Version,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    // TODO: Make private?
    pub id: Value,
}

/// An error code
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct RPCError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// A response to RPC
#[derive(Debug, Deserialize, Clone, PartialEq)]
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

/// A notification (doesn't expect an answer)
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Notification {
    jsonrpc: Version,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

/// One message of the JSON RPC protocol
///
/// One message, directly mapped from the structures of the protocol. See the
/// [specification](http://www.jsonrpc.org/specification) for more details.
///
/// Since the protocol allows one endpoint to be both client and server at the same time, the
/// message can decode and encode both directions of the protocol.
///
/// The `Unmatched` variant is for cases when the message that arrived is valid JSON, but doesn't
/// match the protocol. It allows for handling these non-fatal errors on higher level than the
/// parser.
///
/// It can be serialized and deserialized, or converted to and from a string.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    Request(Request),
    Response(Response),
    Notification(Notification),
    Batch(Vec<Message>),
    Unmatched(Value),
}

impl Message {
    /// A constructor for a request.
    ///
    /// The ID is auto-generated.
    pub fn request(method: String, params: Option<Value>) -> Self {
        Message::Request(Request {
            jsonrpc: Version,
            method: method,
            params: params,
            // TODO!
            id: Value::Null,
        })
    }
    /// A constructor for a notification.
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

macro_rules! deser_branch {
    ($src:expr, $branch:ident) => {
        match from_value($src.clone()) {
            Ok(parsed) => return Message::$branch(parsed),
            Err(_) => (),
        }
    };
}

// TODO: This must be possible to do in less wasteful way. The cloning is stupid :-(
// There seems to be a feature coming in a future release that'd make all this unnecessary.
// https://github.com/serde-rs/serde/pull/739.
impl From<Value> for Message {
    fn from(v: Value) -> Self {
        // Try decoding it by each branch in sequence, taking the first one that matches
        deser_branch!(v, Request);
        deser_branch!(v, Response);
        deser_branch!(v, Notification);
        deser_branch!(v, Batch);
        Message::Unmatched(v)
    }
}

impl Deserialize for Message {
    fn deserialize<D: Deserializer>(deserializer: D) -> Result<Self, D::Error> {
        // Read it as a JSON (delegate the deserialization)
        let preparsed: Value = Deserialize::deserialize(deserializer)?;
        // Convert it
        Ok(Self::from(preparsed))
    }
}

impl FromStr for Message {
    type Err = ::serde_json::error::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ::serde_json::de::from_str(s)
    }
}

impl Into<String> for Message {
    fn into(self) -> String {
        ::serde_json::ser::to_string(&self).unwrap()
    }
}
