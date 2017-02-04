//! A JSON-RPC 2.0 messages
//!
//! The main entrypoint here is the [Message](enum.Message.html). The others are just building
//! blocks and you should generally work with `Message` instead.

use std::str::FromStr;

use serde::ser::{Serialize, Serializer, SerializeStruct};
use serde::de::{Deserialize, Deserializer, Unexpected, Error};
use serde_json::Value;
use uuid::Uuid;

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
#[serde(deny_unknown_fields)]
pub struct Request {
    jsonrpc: Version,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    pub id: Value,
}

/// An error code
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RPCError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// A response to RPC
#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    jsonrpc: Version,
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireResponse {
    // It is actually used to eat and sanity check the deserialized text
    #[allow(dead_code)]
    jsonrpc: Version,
    result: Option<Value>,
    error: Option<RPCError>,
    id: Value,
}

// Implementing deserialize is hard. We sidestep the difficulty by deserializing a similar
// structure that directly corresponds to whatever is on the wire and then convert it to our more
// convenient representation.
impl Deserialize for Response {
    #[allow(unreachable_code)] // For that unreachable below
    fn deserialize<D: Deserializer>(deserializer: D) -> Result<Self, D::Error> {
        let wr: WireResponse = Deserialize::deserialize(deserializer)?;
        let result = match (wr.result, wr.error) {
            (Some(res), None) => Ok(res),
            (None, Some(err)) => Err(err),
            _ => {
                return Err(D::Error::custom("Either 'error' or 'result' is expected, but not both"));
                // A trick to make the compiler accept this branch
                unreachable!();
            },
        };
        Ok(Response {
            jsonrpc: Version,
            result: result,
            id: wr.id,
        })
    }
}

/// A notification (doesn't expect an answer)
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
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
///
/// The `Batch` variant is supposed to be created directly, without a constructor. The `Unmatched`
/// is something you may get from parsing but it is not expected you'd need to create it (though it
/// can be created directly as well).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
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
            id: Value::String(Uuid::new_v4().hyphenated().to_string()),
        })
    }
    /// Answer the request with a (positive) reply.
    ///
    /// The ID is taken from the request.
    pub fn reply(request: &Request, reply: Value) -> Self {
        Message::Response(Response {
            jsonrpc: Version,
            result: Ok(reply),
            id: request.id.clone(),
        })
    }
    /// Answer the request with an error.
    ///
    /// The ID is taken from the request and the error structure is constructed.
    pub fn error(request: &Request, code: i64, message: String, data: Option<Value>) -> Self {
        Message::Response(Response {
            jsonrpc: Version,
            result: Err(RPCError {
                code: code,
                message: message,
                data: data,
            }),
            id: request.id.clone(),
        })
    }
    /// Create an error without a request.
    ///
    /// Create a top-level/free-standing error (one without an ID). This is the required answer for
    /// less serious protocol errors.
    pub fn top_error(code: i64, message: String, data: Option<Value>) -> Self {
        Message::Response(Response {
            jsonrpc: Version,
            result: Err(RPCError {
                code: code,
                message: message,
                data: data,
            }),
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    /// A helper for message_one
    fn message_one(input: &str, expected: &Message) {
        let parsed: Message = input.parse().unwrap();
        assert_eq!(*expected, parsed);
    }

    /// Test serialization and deserialization of the Message
    ///
    /// We first deserialize it from a string. That way we check deserialization works.
    /// But since serialization doesn't have to produce the exact same result (order, spaces, …),
    /// we then serialize and deserialize the thing again and check it matches.
    #[test]
    fn message_serde() {
        // A request without parameters
        message_one(r#"{"jsonrpc": "2.0", "method": "call", "id": 1}"#,
                    &Message::Request(Request {
                        jsonrpc: Version,
                        method: "call".to_owned(),
                        params: None,
                        id: json!(1),
                    }));
        // A request with parameters
        message_one(r#"{"jsonrpc": "2.0", "method": "call", "params": [1, 2, 3], "id": 2}"#,
                    &Message::Request(Request {
                        jsonrpc: Version,
                        method: "call".to_owned(),
                        params: Some(json!([1, 2, 3])),
                        id: json!(2),
                    }));
        // A notification (with parameters)
        message_one(r#"{"jsonrpc": "2.0", "method": "notif", "params": {"x": "y"}}"#,
                    &Message::Notification(Notification {
                        jsonrpc: Version,
                        method: "notif".to_owned(),
                        params: Some(json!({"x": "y"})),
                    }));
        // A successful response
        message_one(r#"{"jsonrpc": "2.0", "result": 42, "id": 3}"#,
                    &Message::Response(Response {
                        jsonrpc: Version,
                        result: Ok(json!(42)),
                        id: json!(3),
                    }));
        // An error
        message_one(r#"{"jsonrpc": "2.0", "error": {"code": 42, "message": "Wrong!"}, "id": null}"#,
                    &Message::Response(Response {
                        jsonrpc: Version,
                        result: Err(RPCError {
                            code: 42,
                            message: "Wrong!".to_owned(),
                            data: None,
                        }),
                        id: Value::Null,
                    }));
        // A batch
        message_one(r#"[
                {"jsonrpc": "2.0", "method": "notif"},
                {"jsonrpc": "2.0", "method": "call", "id": 42}
            ]"#,
                    &Message::Batch(vec![
                Message::Notification(Notification {
                    jsonrpc: Version,
                    method: "notif".to_owned(),
                    params: None,
                }),
                Message::Request(Request {
                    jsonrpc: Version,
                    method: "call".to_owned(),
                    params: None,
                    id: json!(42),
                }),
            ]));
    }

    /// A helper for the `broken` test.
    ///
    /// Check that the given JSON string parses, but is not recognized as a valid RPC message.
    fn broken_one(input: &str) {
        let msg = input.parse().unwrap();
        match &msg {
            &Message::Unmatched(_) => (),
            _ => panic!("{} recognized as an RPC message: {:?}!", input, msg),
        }
    }

    /// Test things that are almost but not entirely JSONRPC are rejected
    ///
    /// The reject is done by returning it as Unmatched.
    #[test]
    fn broken() {
        // Missing the version
        broken_one(r#"{"method": "notif"}"#);
        // Wrong version
        broken_one(r#"{"jsonrpc": 2.0, "method": "notif"}"#);
        // A response with both result and error
        broken_one(r#"{"jsonrpc": "2.0", "result": 42, "error": {"code": 42, "message": "Wrong!"}, "id": 1}"#);
        // A response without an id
        broken_one(r#"{"jsonrpc": "2.0", "result": 42}"#);
        // An extra field
        broken_one(r#"{"jsonrpc": "2.0", "method": "weird", "params": 42, "others": 43, "id": 2}"#);
        // Something completely different
        broken_one(r#"{"x": [1, 2, 3]}"#);
    }
}
