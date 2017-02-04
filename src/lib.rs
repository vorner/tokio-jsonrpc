extern crate serde;
// We use the json! macro only in the tests
#[allow(unused_imports)]
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate uuid;

pub mod message;
