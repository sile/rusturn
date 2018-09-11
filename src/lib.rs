extern crate fibers;
extern crate futures;
extern crate rand;
extern crate rustun;
extern crate stun_codec;
#[macro_use]
extern crate trackable;

pub use rustun::{Error, ErrorKind, Result};

//pub mod server;
pub mod client;
pub mod types;

pub const DEFAULT_LIFETIME_SECONDS: u64 = 10 * 60;
