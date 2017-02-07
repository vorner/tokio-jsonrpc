// Copyright 2017 tokio-jsonrpc Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.
// Copyright (c) 2017 Michal 'vorner' Vaner <vorner@vorner.cz>

//! The codecs to encode and decode messages from a stream of bytes.

// TODO: Have both line-separated and object separated codecs. The first can detect syntax errors,
// while the other can decode multiline messages or messages on single line.

use std::io::{Result as IoResult, Error, ErrorKind};

use tokio_core::io::{Codec as TokioCodec, EasyBuf};
use serde_json::de::from_slice;
use serde_json::ser::to_vec;
use serde_json::error::Error as SerdeError;

use message::Message;

/// A helper to wrap the error
fn err_map(e: SerdeError) -> Error {
    Error::new(ErrorKind::Other, e)
}

/// A codec working with JSONRPC 2.0 messages.
///
/// This produces or encodes [Message](../message/enum.Message.hmtl). It takes the JSON object boundaries,
/// so it works with both newline-separated and object-separated encoding. It produces
/// newline-separated stream, which is more generic.
pub struct Codec;

impl TokioCodec for Codec {
    type In = Message;
    type Out = Message;
    fn decode(&mut self, buf: &mut EasyBuf) -> IoResult<Option<Message>> {
        // TODO: Use object boundary instead of newlines. This waits for
        // https://github.com/serde-rs/json/pull/212 or for being able to
        // distinguish EOF errors from the others for the trick in
        // https://github.com/serde-rs/json/issues/183.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode() {
        let mut output = Vec::new();
        let mut codec = Codec;
        codec.encode(Message::notification("notif".to_owned(), None), &mut output).unwrap();
        assert_eq!(Vec::from(&b"{\"jsonrpc\":\"2.0\",\"method\":\"notif\"}\n"[..]), output);
    }

    #[test]
    fn decode() {
        fn one(input: &[u8], rest: &[u8]) -> IoResult<Option<Message>> {
            let mut codec = Codec;
            let mut buf = EasyBuf::new();
            buf.get_mut().extend_from_slice(input);
            let result = codec.decode(&mut buf);
            assert_eq!(rest, buf.as_slice());
            result
        }

        // TODO: We currently have to terminate the records by newline, but that's a temporary
        // problem. Once that is solved, have some tests without the newline as well. Also, test
        // some messages that don't have a newline, but have a syntax error, so we know we abort
        // soon enough.

        let notif = Message::notification("notif".to_owned(), None);
        let msgstring = Vec::from(&b"{\"jsonrpc\":\"2.0\",\"method\":\"notif\"}\n"[..]);
        // A single message, nothing is left
        assert_eq!(one(&msgstring, b"").unwrap(), Some(notif.clone()));
        // The first message is decoded, the second stays in the buffer
        let mut twomsgs = msgstring.clone();
        twomsgs.extend_from_slice(&msgstring);
        assert_eq!(one(&twomsgs, &msgstring).unwrap(), Some(notif.clone()));
        // The second message is incomplete, but stays there
        let incomplete = Vec::from(&br#"{"jsonrpc": "2.0", "method":""#[..]);
        let mut oneandhalf = msgstring.clone();
        oneandhalf.extend_from_slice(&incomplete);
        assert_eq!(one(&oneandhalf, &incomplete).unwrap(), Some(notif.clone()));
        // An incomplete message ‒ nothing gets out and everything stays
        assert_eq!(one(&incomplete, &incomplete).unwrap(), None);
        // A syntax error is reported as an error (and eaten, but that's no longer interesting)
        assert!(one(b"{]\n", b"").is_err());
    }
}
