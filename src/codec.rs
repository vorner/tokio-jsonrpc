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

use std::io::{Error, ErrorKind, Result as IoResult};

use tokio_io::codec::{Decoder, Encoder};
use bytes::{BufMut, BytesMut};
use serde_json::ser::to_vec;
use serde_json::error::Error as SerdeError;

use message::{Message, Parsed, from_slice, from_str};

/// A helper to wrap the error
fn err_map(e: SerdeError) -> Error {
    Error::new(ErrorKind::Other, e)
}

/// A helper trait to unify `Line` and `DirtyLine`
trait PositionCache {
    fn position(&mut self) -> &mut usize;
}

/// An encoding function reused by `Line` and `DirtyLine`
fn encode_codec(msg: &Message, buf: &mut BytesMut) -> IoResult<()> {
    let encoded = to_vec(&msg).map_err(err_map)?;
    // As discovered the hard way, we must not overwrite buf, but append to it.
    buf.reserve(encoded.len() + 1);
    buf.put_slice(&encoded);
    buf.put(b'\n');
    Ok(())
}

fn decode_codec<Cache, Convert>(cache: &mut Cache, buf: &mut BytesMut, convert: Convert)
                                -> IoResult<Option<Parsed>>
    where Cache: PositionCache,
          Convert: FnOnce(&[u8]) -> Parsed
{
    // Where did we stop scanning before? Scan only the new part
    let start_pos = cache.position();
    if let Some(i) = buf[*start_pos..].iter().position(|&b| b == b'\n') {
        let end_pos = *start_pos + i;
        let line = buf.split_to(end_pos);
        buf.split_to(1);
        // We'll start from the beginning next time.
        *start_pos = 0;
        Ok(Some(convert(&line)))
    } else {
        // Mark where we ended scanning.
        *start_pos = buf.len();
        Ok(None)
    }
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
    /// A constructor
    pub fn new() -> Self {
        Line(0)
    }
}

impl PositionCache for Line {
    fn position(&mut self) -> &mut usize {
        &mut self.0
    }
}

impl Encoder for Line {
    type Item = Message;
    type Error = Error;
    fn encode(&mut self, msg: Message, buf: &mut BytesMut) -> IoResult<()> {
        encode_codec(&msg, buf)
    }
}

impl Decoder for Line {
    type Item = Parsed;
    type Error = Error;
    fn decode(&mut self, src: &mut BytesMut) -> IoResult<Option<Parsed>> {
        decode_codec(self, src, from_slice)
    }
}

/// A codec working with JSONRPC 2.0 messages on top of badly encoded utf-8.
///
/// This works like the [Line](struct.Line.html) codec. However, it can cope with the input not
/// being valid utf-8. That is arguably broken, nevertheless found in the wild and sometimes the
/// only thing left to be done is to cope with it. This copes with it by running the input through
/// the `String::from_utf8_lossy` conversion, effectively replacing anything that is not valid with
/// these special utf-8 WTF question marks (U+FFFD).
///
/// In contrast, Line errors on such invalid inputs. Encoding is the same for both codecs, however.
#[derive(Debug, Default)]
pub struct DirtyLine(usize);

impl DirtyLine {
    /// A constructor
    pub fn new() -> Self {
        DirtyLine(0)
    }
}

impl PositionCache for DirtyLine {
    fn position(&mut self) -> &mut usize {
        &mut self.0
    }
}

impl Decoder for DirtyLine {
    type Item = Parsed;
    type Error = Error;
    fn decode(&mut self, src: &mut BytesMut) -> IoResult<Option<Parsed>> {
        decode_codec(self,
                     src,
                     |bytes| from_str(String::from_utf8_lossy(bytes).as_ref()))
    }
}

impl Encoder for DirtyLine {
    type Item = Message;
    type Error = Error;
    fn encode(&mut self, msg: Message, buf: &mut BytesMut) -> IoResult<()> {
        encode_codec(&msg, buf)
    }
}

/// A codec working with JSONRPC 2.0 messages.
///
/// This produces or encodes [Message](../message/enum.Message.hmtl). It takes the JSON object
/// boundaries, so it works with both newline-separated and object-separated encoding. It produces
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
        let mut output = BytesMut::with_capacity(10);
        let mut codec = Line::new();
        let msg = Message::notification("notif".to_owned(), None);
        let encoded = BytesMut::from(&b"{\"jsonrpc\":\"2.0\",\"method\":\"notif\"}\n"[..]);
        codec.encode(msg.clone(), &mut output).unwrap();
        assert_eq!(encoded, output);
        let mut dirty_codec = DirtyLine::new();
        output.clear();
        dirty_codec.encode(msg, &mut output).unwrap();
        assert_eq!(encoded, output);
    }

    fn get_buf(input: &[u8]) -> BytesMut {
        BytesMut::from(input)
    }

    #[test]
    fn decode() {
        fn one(input: &[u8], rest: &[u8]) -> IoResult<Option<Parsed>> {
            let mut codec = Line::new();
            let mut buf = get_buf(input);
            let result = codec.decode(&mut buf);
            assert_eq!(rest, &buf);
            // On all the valid inputs, DirtyLine should act the same as Line
            let mut dirty_codec = DirtyLine::new();
            let mut buf = get_buf(input);
            let dirty = dirty_codec.decode(&mut buf);
            assert_eq!(rest, &buf);
            assert_eq!(result.as_ref().unwrap(), dirty.as_ref().unwrap());
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
        assert_eq!(one(&oneandhalf, &incomplete).unwrap(),
                   Some(Ok(notif.clone())));
        // An incomplete message ‒ nothing gets out and everything stays
        assert_eq!(one(&incomplete, &incomplete).unwrap(), None);
        // A syntax error is reported as an error (and eaten, but that's no longer interesting)
        match one(b"{]\n", b"") {
            Ok(Some(Err(Broken::SyntaxError(_)))) => (),
            other => panic!("Something unexpected: {:?}", other),
        };
    }

    /// Test with invalid utf-8 in a string
    #[test]
    fn decode_nonunicode() {
        let broken_input = b"{\"jsonrpc\":\"2.0\",\"method\":\"Hello \xF0\x90\x80World\"}\n";
        let mut codec = Line::new();
        let mut buf = get_buf(broken_input);
        // The ordinary line codec gives up
        let result = codec.decode(&mut buf).unwrap();
        match result {
            Some(Err(Broken::SyntaxError(_))) => (),
            other => panic!("Something unexpected: {:?}", other),
        };
        buf = get_buf(broken_input);
        // But the dirty one just keeps going on
        let mut dirty = DirtyLine::new();
        let result = dirty.decode(&mut buf).unwrap();
        assert_eq!(result,
                   Some(Ok(Message::notification("Hello �World".to_owned(), None))));
    }
}
