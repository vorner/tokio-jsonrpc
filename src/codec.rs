// Copyright 2017 tokio-jsonrpc Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

//! The codecs to encode and decode messages from a stream of bytes.
//!
//! You can choose to use either line separated one ([Line](struct.Line.html)) or
//! boundary separated one ([Boundary](struct.Boundary.html)). The first one needs the
//! messages to be separated by newlines and not to contain newlines in their representation. On
//! the other hand, it can recover from syntax error in a message and you can respond with an error
//! instead of terminating the connection.

use std::io::{Result as IoResult, Error, ErrorKind};

use tokio_core::io::{Codec, EasyBuf};
use serde_json::ser::to_vec;
use serde_json::error::Error as SerdeError;

use message::{Message, Parsed, from_slice};

/// A helper to wrap the error
fn err_map(e: SerdeError) -> Error {
    Error::new(ErrorKind::Other, e)
}

/// A codec working with JSONRPC 2.0 messages.
///
/// This produces or encodes [Message](../message/enum.Message.hmtl). It separates the records by
/// newlines, so it can recover from syntax error.s
///
/// Note that the produced items is a `Result`, to allow not terminating the stream on
/// protocol-level errors.
#[derive(Debug, Default)]
pub struct Line(usize);

impl Line {
    pub fn new() -> Self {
        Line(0)
    }
}

impl Codec for Line {
    type In = Parsed;
    type Out = Message;
    fn decode(&mut self, buf: &mut EasyBuf) -> IoResult<Option<Parsed>> {
        // Where did we stop scanning before? Scan only the new part
        let start_pos = self.0;
        if let Some(i) = buf.as_slice()[start_pos..].iter().position(|&b| b == b'\n') {
            let end_pos = start_pos + i;
            let line = buf.drain_to(end_pos);
            buf.drain_to(1);
            // We'll start from the beginning next time.
            self.0 = 0;
            Ok(Some(from_slice(line.as_slice())))
        } else {
            // Mark where we ended scanning.
            self.0 = buf.len();
            Ok(None)
        }
    }
    fn encode(&mut self, msg: Message, buf: &mut Vec<u8>) -> IoResult<()> {
        *buf = to_vec(&msg).map_err(err_map)?;
        buf.push(b'\n');
        Ok(())
    }
}

/// A codec working with JSONRPC 2.0 messages.
///
/// This produces or encodes [Message](../message/enum.Message.hmtl). It takes the JSON object boundaries,
/// so it works with both newline-separated and object-separated encoding. It produces
/// newline-separated stream, which is more generic.
///
/// TODO: This is not implemented yet.
pub struct Boundary;

#[cfg(test)]
mod tests {
    use super::*;
    use message::Broken;

    #[test]
    fn encode() {
        let mut output = Vec::new();
        let mut codec = Line::new();
        codec.encode(Message::notification("notif".to_owned(), None), &mut output).unwrap();
        assert_eq!(Vec::from(&b"{\"jsonrpc\":\"2.0\",\"method\":\"notif\"}\n"[..]), output);
    }

    #[test]
    fn decode() {
        fn one(input: &[u8], rest: &[u8]) -> IoResult<Option<Parsed>> {
            let mut codec = Line::new();
            let mut buf = EasyBuf::new();
            buf.get_mut().extend_from_slice(input);
            let result = codec.decode(&mut buf);
            assert_eq!(rest, buf.as_slice());
            result
        }

        let notif = Message::notification("notif".to_owned(), None);
        let msgstring = Vec::from(&b"{\"jsonrpc\":\"2.0\",\"method\":\"notif\"}\n"[..]);
        // A single message, nothing is left
        assert_eq!(one(&msgstring, b"").unwrap(), Some(Ok(notif.clone())));
        // The first message is decoded, the second stays in the buffer
        let mut twomsgs = msgstring.clone();
        twomsgs.extend_from_slice(&msgstring);
        assert_eq!(one(&twomsgs, &msgstring).unwrap(), Some(Ok(notif.clone())));
        // The second message is incomplete, but stays there
        let incomplete = Vec::from(&br#"{"jsonrpc": "2.0", "method":""#[..]);
        let mut oneandhalf = msgstring.clone();
        oneandhalf.extend_from_slice(&incomplete);
        assert_eq!(one(&oneandhalf, &incomplete).unwrap(), Some(Ok(notif.clone())));
        // An incomplete message ‒ nothing gets out and everything stays
        assert_eq!(one(&incomplete, &incomplete).unwrap(), None);
        // A syntax error is reported as an error (and eaten, but that's no longer interesting)
        match one(b"{]\n", b"") {
            Ok(Some(Err(Broken::SyntaxError(_)))) => (),
            other => panic!("Something unexpected: {:?}", other),
        };
    }
}
