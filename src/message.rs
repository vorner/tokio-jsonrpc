// Copyright 2017 tokio-jsonrpc Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

//! JSON-RPC 2.0 messages.
//!
//! The main entrypoint here is the [Message](enum.Message.html). The others are just building
//! blocks and you should generally work with `Message` instead.

use serde::ser::{Serialize, SerializeStruct, Serializer};
use serde::de::{Deserialize, Deserializer, Error, Unexpected};
use serde_json::{self, Value, to_value};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

/// An RPC method params
///
/// Not all json is a valid RPC params.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum Params {
    Positional(Vec<Value>),
    Named(serde_json::Map<String, Value>),
}

impl Params {
    /// Wether or not there are any params
    pub fn is_empty(&self) -> bool {
        match *self {
            Params::Positional(ref args) => args.is_empty(),
            Params::Named(ref fields) => fields.is_empty(),
        }
    }

    pub fn into_value(self) -> Value {
        match self {
            Params::Positional(xs) => Value::Array(xs),
            Params::Named(x) => Value::Object(x),
        }
    }
}

// Helper fuction for skipping the params field serialization when there are no params
pub fn params_empty(x: &Option<Params>) -> bool {
    match *x {
        Some(ref x) => x.is_empty(),
        None => true,
    }
}

// TODO: how do we sanitize uses of `json_internal!()`? check the rocket crate
// TODO: missing docs
#[macro_export]
macro_rules! params {

    //////////////////////////////////////////////////////////////////////////
    // The main implementation.
    //
    // Must be invoked as: params!($($json)+)
    //////////////////////////////////////////////////////////////////////////

    () => {
        None
    };

    // Review: do we want to use this to send a `"params": null` as opposed to skipping it?
    //         params!() or None can be the skip while params!(null) will actually send null
    //         Notes:
    //           - Params::Null needs to be added
    //           - `None` and `Some(Params::Null)` are "almost" equivalent on server side
    (null) => {
        None
    };

    (true) => {
        Some($crate::Params::Positional(vec![$crate::macro_exports::Value::Bool(true)]))
    };

    (false) => {
        Some($crate::Params::Positional(vec![$crate::macro_exports::Value::Bool(false)]))
    };

    ([]) => {
        {
            Some($crate::Params::Positional(vec![]))
        }
    };

    ([ $($tt:tt)+ ]) => {
        {
            match $crate::macro_exports::Value::Array(json_internal!(@array [] $($tt)+)) {
                $crate::macro_exports::Value::Array(xs) => Some($crate::Params::Positional(xs)),
                _ => unreachable!(),
            }
        }
    };

    ({}) => {
        {
            Some($crate::Params::Named($crate::Map::new()))
        }
    };

    ({ $($tt:tt)+ }) => {
        {
            let mut object = $crate::macro_exports::Map::new();
            json_internal!(@object object () $($tt)+);
            Some($crate::Params::Named(object))
        }
    };

    // Force structs to be serialized as positional
    (pos $other:expr) => {
        match $crate::macro_exports::to_value(&$other).unwrap() {
            // Turn the object into positional discarding the names
            $crate::macro_exports::Value::Object(x) => {
                let mut xs = Vec::with_capacity(x.len());
                for (_,v) in x {
                    xs.push(v);
                }
                Some($crate::Params::Positional(xs))
            },
            $crate::macro_exports::Value::Array(xs) =>
                Some($crate::Params::Positional(xs)),
            value => Some($crate::Params::Positional(vec![value])),
        }
    };

    // Any Serialize type: numbers, strings, struct literals, variables etc.
    // Must be below every other rule.
    ($other:expr) => {
        match $crate::macro_exports::to_value(&$other).unwrap() {
            $crate::macro_exports::Value::Object(x) =>
                Some($crate::Params::Named(x)),
            $crate::macro_exports::Value::Array(xs) =>
                Some($crate::Params::Positional(xs)),
            value => Some($crate::Params::Positional(vec![value])),
        }
    };
}

/// An RPC request.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Request {
    jsonrpc: Version,
    pub method: String,
    #[serde(skip_serializing_if = "params_empty")]
    pub params: Option<Params>,
    pub id: Value,
}

impl Request {
    /// Answer the request with a (positive) reply.
    ///
    /// The ID is taken from the request.
    pub fn reply(&self, reply: Value) -> Message {
        Message::Response(Response {
                              jsonrpc: Version,
                              result: Ok(reply),
                              id: self.id.clone(),
                          })
    }
    /// Answer the request with an error.
    pub fn error(&self, error: RpcError) -> Message {
        Message::Response(Response {
                              jsonrpc: Version,
                              result: Err(error),
                              id: self.id.clone(),
                          })
    }
}

/// An error code.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl RpcError {
    /// A generic constructor.
    ///
    /// Mostly for completeness, doesn't do anything but filling in the corresponding fields.
    pub fn new(code: i64, message: String, data: Option<Value>) -> Self {
        RpcError {
            code: code,
            message: message,
            data: data,
        }
    }
    /// Create an Invalid Param error.
    pub fn invalid_params(msg: Option<String>) -> Self {
        RpcError::new(-32602, "Invalid params".to_owned(), msg.map(Value::String))
    }
    /// Create a server error.
    pub fn server_error<E: Serialize>(e: Option<E>) -> Self {
        RpcError::new(-32000,
                      "Server error".to_owned(),
                      e.map(|v| to_value(v).expect("Must be representable in JSON")))
    }
    /// Create an invalid request error.
    pub fn invalid_request() -> Self {
        RpcError::new(-32600, "Invalid request".to_owned(), None)
    }
    /// Create a parse error.
    pub fn parse_error(e: String) -> Self {
        RpcError::new(-32700, "Parse error".to_owned(), Some(Value::String(e)))
    }
    /// Create a method not found error.
    pub fn method_not_found(method: String) -> Self {
        RpcError::new(-32601,
                      "Method not found".to_owned(),
                      Some(Value::String(method)))
    }
}

/// A response to an RPC.
///
/// It is created by the methods on [Request](struct.Request.html).
#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    jsonrpc: Version,
    pub result: Result<Value, RpcError>,
    pub id: Value,
}

impl Serialize for Response {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sub = serializer.serialize_struct("Response", 3)?;
        sub.serialize_field("jsonrpc", &self.jsonrpc)?;
        match self.result {
            Ok(ref value) => sub.serialize_field("result", value),
            Err(ref err) => sub.serialize_field("error", err),
        }?;
        sub.serialize_field("id", &self.id)?;
        sub.end()
    }
}

/// Deserializer for `Option<Value>` that produces `Some(Value::Null)`.
///
/// The usual one produces None in that case. But we need to know the difference between
/// `{x: null}` and `{}`.
fn some_value<D: Deserializer>(deserializer: D) -> Result<Option<Value>, D::Error> {
    Deserialize::deserialize(deserializer).map(Some)
}

/// A helper trick for deserialization.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireResponse {
    // It is actually used to eat and sanity check the deserialized text
    #[allow(dead_code)]
    jsonrpc: Version,
    // Make sure we accept null as Some(Value::Null), instead of going to None
    #[serde(default, deserialize_with = "some_value")]
    result: Option<Value>,
    error: Option<RpcError>,
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
                let err = D::Error::custom("Either 'error' or 'result' is expected, but not both");
                return Err(err);
            },
        };
        Ok(Response {
               jsonrpc: Version,
               result: result,
               id: wr.id,
           })
    }
}

/// A notification (doesn't expect an answer).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Notification {
    jsonrpc: Version,
    pub method: String,
    #[serde(skip_serializing_if = "params_empty")]
    pub params: Option<Params>,
}

/// One message of the JSON RPC protocol.
///
/// One message, directly mapped from the structures of the protocol. See the
/// [specification](http://www.jsonrpc.org/specification) for more details.
///
/// Since the protocol allows one endpoint to be both client and server at the same time, the
/// message can decode and encode both directions of the protocol.
///
/// The `Batch` variant is supposed to be created directly, without a constructor.
///
/// The `UnmatchedSub` variant is used when a request is an array and some of the subrequests
/// aren't recognized as valid json rpc 2.0 messages. This is never returned as a top-level
/// element, it is returned as `Err(Broken::Unmatched)`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Message {
    /// An RPC request.
    Request(Request),
    /// A response to a Request.
    Response(Response),
    /// A notification.
    Notification(Notification),
    /// A batch of more requests or responses.
    ///
    /// The protocol allows bundling multiple requests, notifications or responses to a single
    /// message.
    ///
    /// This variant has no direct constructor and is expected to be constructed manually.
    Batch(Vec<Message>),
    /// An unmatched sub entry in a `Batch`.
    ///
    /// When there's a `Batch` and an element doesn't comform to the JSONRPC 2.0 format, that one
    /// is represented by this. This is never produced as a top-level value when parsing, the
    /// `Err(Broken::Unmatched)` is used instead. It is not possible to serialize.
    #[serde(skip_serializing)]
    UnmatchedSub(Value),
}

impl Message {
    /// A constructor for a request.
    ///
    /// The ID is auto-generated.
    pub fn request(method: String, params: Option<Params>) -> Self {
        Message::Request(Request {
                             jsonrpc: Version,
                             method: method,
                             params: params,
                             id: Value::String(Uuid::new_v4().hyphenated().to_string()),
                         })
    }
    /// Create a top-level error (without an ID).
    pub fn error(error: RpcError) -> Self {
        Message::Response(Response {
                              jsonrpc: Version,
                              result: Err(error),
                              id: Value::Null,
                          })
    }
    /// A constructor for a notification.
    pub fn notification(method: String, params: Option<Params>) -> Self {
        Message::Notification(Notification {
                                  jsonrpc: Version,
                                  method: method,
                                  params: params,
                              })
    }
}

/// A broken message.
///
/// Protocol-level errors.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Broken {
    /// It was valid JSON, but doesn't match the form of a JSONRPC 2.0 message.
    Unmatched(Value),
    /// Invalid JSON.
    #[serde(skip_deserializing)]
    SyntaxError(String),
}

impl Broken {
    /// Generate an appropriate error message.
    ///
    /// The error message for these things are specified in the RFC, so this just creates an error
    /// with the right values.
    pub fn reply(&self) -> Message {
        match *self {
            Broken::Unmatched(_) => Message::error(RpcError::invalid_request()),
            Broken::SyntaxError(ref e) => Message::error(RpcError::parse_error(e.clone())),
        }
    }
}

/// A trick to easily deserialize and detect valid JSON, but invalid Message.
#[derive(Deserialize)]
#[serde(untagged)]
enum WireMessage {
    Message(Message),
    Broken(Broken),
}

pub type Parsed = Result<Message, Broken>;

/// Read a [Message](enum.Message.html) from a slice.
///
/// Invalid JSON or JSONRPC messages are reported as [Broken](enum.Broken.html).
pub fn from_slice(s: &[u8]) -> Parsed {
    match ::serde_json::de::from_slice(s) {
        Ok(WireMessage::Message(Message::UnmatchedSub(value))) => Err(Broken::Unmatched(value)),
        Ok(WireMessage::Message(m)) => Ok(m),
        Ok(WireMessage::Broken(b)) => Err(b),
        // Other errors can't happen right now, when we have the slice
        Err(e) => Err(Broken::SyntaxError(format!("{}", e))),
    }
}

/// Read a [Message](enum.Message.html) from a string.
///
/// Invalid JSON or JSONRPC messages are reported as [Broken](enum.Broken.html).
pub fn from_str(s: &str) -> Parsed {
    from_slice(s.as_bytes())
}

impl Into<String> for Message {
    fn into(self) -> String {
        ::serde_json::ser::to_string(&self).unwrap()
    }
}

impl Into<Vec<u8>> for Message {
    fn into(self) -> Vec<u8> {
        ::serde_json::ser::to_vec(&self).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use serde_json::ser::to_vec;
    use serde_json::de::from_slice;

    #[derive(Serialize)]
    struct TestParam {
        x: usize,
        y: usize,
    }

    /// Test serialization and deserialization of the Message
    ///
    /// We first deserialize it from a string. That way we check deserialization works.
    /// But since serialization doesn't have to produce the exact same result (order, spaces, …),
    /// we then serialize and deserialize the thing again and check it matches.
    #[test]
    fn message_serde() {
        // A helper for running one message test
        fn one(input: &str, expected: &Message) {
            let parsed: Message = from_str(input).unwrap();
            assert_eq!(*expected, parsed);
            let serialized = to_vec(&parsed).unwrap();
            let deserialized: Message = from_slice(&serialized).unwrap();
            assert_eq!(parsed, deserialized);
        }

        // A request without parameters
        one(r#"{"jsonrpc": "2.0", "method": "call", "id": 1}"#,
            &Message::Request(Request {
                                  jsonrpc: Version,
                                  method: "call".to_owned(),
                                  params: None,
                                  id: json!(1),
                              }));
        // A request with `"params": null`
        // Note: This is **NOT** part of the specification, but we will honor it as the
        // same as not being present.
        one(r#"{"jsonrpc": "2.0", "method": "call", "params": null, "id": 1}"#,
            &Message::Request(Request {
                                  jsonrpc: Version,
                                  method: "call".to_owned(),
                                  params: None,
                                  id: json!(1),
                              }));
        // A request with parameters
        one(r#"{"jsonrpc": "2.0", "method": "call", "params": [1, 2, 3], "id": 2}"#,
            &Message::Request(Request {
                                  jsonrpc: Version,
                                  method: "call".to_owned(),
                                  params: params!([1, 2, 3]),
                                  id: json!(2),
                              }));
        // A request with a struct parameter (default: named)
        one(r#"{"jsonrpc": "2.0", "method": "call", "params": {"x": 2, "y": 7}, "id": 2}"#,
            &Message::Request(Request {
                                  jsonrpc: Version,
                                  method: "call".to_owned(),
                                  params: params!(TestParam { x: 2, y: 7 }),
                                  id: json!(2),
                              }));
        // A request with a struct parameter (in positional form)
        one(r#"{"jsonrpc": "2.0", "method": "call", "params": [2, 7], "id": 2}"#,
            &Message::Request(Request {
                                  jsonrpc: Version,
                                  method: "call".to_owned(),
                                  params: params!(pos TestParam { x: 2, y: 7 }),
                                  id: json!(2),
                              }));
        // A request with a literal parameter
        one(r#"{"jsonrpc": "2.0", "method": "call", "params": [21], "id": 2}"#,
            &Message::Request(Request {
                                  jsonrpc: Version,
                                  method: "call".to_owned(),
                                  params: params!(21),
                                  id: json!(2),
                              }));
        // A notification (with parameters)
        one(r#"{"jsonrpc": "2.0", "method": "notif", "params": {"x": "y"}}"#,
            &Message::Notification(Notification {
                                       jsonrpc: Version,
                                       method: "notif".to_owned(),
                                       params: params!({"x": "y"}),
                                   }));
        // A successful response
        one(r#"{"jsonrpc": "2.0", "result": 42, "id": 3}"#,
            &Message::Response(Response {
                                   jsonrpc: Version,
                                   result: Ok(json!(42)),
                                   id: json!(3),
                               }));
        // A successful response
        one(r#"{"jsonrpc": "2.0", "result": null, "id": 3}"#,
            &Message::Response(Response {
                                   jsonrpc: Version,
                                   result: Ok(Value::Null),
                                   id: json!(3),
                               }));
        // An error
        one(r#"{"jsonrpc": "2.0", "error": {"code": 42, "message": "Wrong!"}, "id": null}"#,
            &Message::Response(Response {
                                   jsonrpc: Version,
                                   result: Err(RpcError::new(42, "Wrong!".to_owned(), None)),
                                   id: Value::Null,
                               }));
        // A batch
        one(r#"[
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
        // Some handling of broken messages inside a batch
        let parsed = from_str(r#"[
                {"jsonrpc": "2.0", "method": "notif"},
                {"jsonrpc": "2.0", "method": "call", "id": 42},
                true
            ]"#)
                .unwrap();
        assert_eq!(Message::Batch(vec![
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
                Message::UnmatchedSub(Value::Bool(true)),
            ]), parsed);
        to_vec(&Message::UnmatchedSub(Value::Null)).unwrap_err();
    }

    /// A helper for the `broken` test.
    ///
    /// Check that the given JSON string parses, but is not recognized as a valid RPC message.

    /// Test things that are almost but not entirely JSONRPC are rejected
    ///
    /// The reject is done by returning it as Unmatched.
    #[test]
    fn broken() {
        // A helper with one test
        fn one(input: &str) {
            let msg = from_str(input);
            match msg {
                Err(Broken::Unmatched(_)) => (),
                _ => panic!("{} recognized as an RPC message: {:?}!", input, msg),
            }
        }

        // Missing the version
        one(r#"{"method": "notif"}"#);
        // Wrong version
        one(r#"{"jsonrpc": 2.0, "method": "notif"}"#);
        // A response with both result and error
        one(r#"{"jsonrpc": "2.0", "result": 42, "error": {"code": 42, "message": "!"}, "id": 1}"#);
        // A response without an id
        one(r#"{"jsonrpc": "2.0", "result": 42}"#);
        // An extra field
        one(r#"{"jsonrpc": "2.0", "method": "weird", "params": [42], "others": 43, "id": 2}"#);
        // Some json param that is *not* valid jsonrpc
        // Valid jsonrpc params are only "not present", an array (positional) or an object (named)
        // Note: notice that present but `null` is not part of the specification
        one(r#"{"jsonrpc": "2.0", "method": "weird", "params": 21, "id": 2}"#);
        // Something completely different
        one(r#"{"x": [1, 2, 3]}"#);

        match from_str(r#"{]"#) {
            Err(Broken::SyntaxError(_)) => (),
            other => panic!("Something unexpected: {:?}", other),
        };
    }

    /// Test some non-trivial aspects of the constructors
    ///
    /// This doesn't have a full coverage, because there's not much to actually test there.
    /// Most of it is related to the ids.
    #[test]
    fn constructors() {
        let msg1 = Message::request("call".to_owned(), params!([1, 2, 3]));
        let msg2 = Message::request("call".to_owned(), params!([1, 2, 3]));
        // They differ, even when created with the same parameters
        assert_ne!(msg1, msg2);
        // And, specifically, they differ in the ID's
        let (req1, req2) = if let (Message::Request(req1), Message::Request(req2)) = (msg1, msg2) {
            assert_ne!(req1.id, req2.id);
            assert!(req1.id.is_string());
            assert!(req2.id.is_string());
            (req1, req2)
        } else {
            panic!("Non-request received");
        };
        let id1 = req1.id.clone();
        // When we answer a message, we get the same ID
        if let Message::Response(ref resp) = req1.reply(json!([1, 2, 3])) {
            assert_eq!(*resp, Response {
                jsonrpc: Version,
                result: Ok(json!([1, 2, 3])),
                id: id1,
            });
        } else {
            panic!("Not a response");
        }
        let id2 = req2.id.clone();
        // The same with an error
        if let Message::Response(ref resp) =
            req2.error(RpcError::new(42, "Wrong!".to_owned(), None)) {
            assert_eq!(*resp, Response {
                jsonrpc: Version,
                result: Err(RpcError::new(42, "Wrong!".to_owned(), None)),
                id: id2,
            });
        } else {
            panic!("Not a response");
        }
        // When we have unmatched, we generate a top-level error with Null id.
        if let Message::Response(ref resp) =
            Message::error(RpcError::new(43, "Also wrong!".to_owned(), None)) {
            assert_eq!(*resp, Response {
                jsonrpc: Version,
                result: Err(RpcError::new(43, "Also wrong!".to_owned(), None)),
                id: Value::Null,
            });
        } else {
            panic!("Not a response");
        }
    }
}
