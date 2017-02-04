extern crate serde;
// We use the json! macro only in the tests
#[cfg_attr(test, macro_use)]
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate uuid;

pub mod message;

pub use message::Message;
