extern crate serde;
// We use the json! macro only in the tests
#[cfg_attr(test, macro_use)]
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate uuid;
extern crate tokio_core;

pub mod message;
pub mod codec;

pub use message::Message;
pub use codec::Codec;
