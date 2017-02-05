//! The codec to encode and decode messages from a stream of bytes.

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
