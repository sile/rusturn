#[macro_use]
extern crate bytecodec;
extern crate fibers;
extern crate futures;
extern crate rand;
extern crate rustun;
#[macro_use]
extern crate stun_codec;
#[macro_use]
extern crate trackable;

pub use rustun::{Error, ErrorKind, Result};

//pub mod server;
pub mod attribute;
pub mod auth;
pub mod channel_data;
// pub mod client;
pub mod client2;
pub mod transport;
pub mod types;

pub const DEFAULT_LIFETIME_SECONDS: u64 = 10 * 60;

#[derive(Debug)]
pub struct AsyncResult<T>(T);
impl<T> futures::Future for AsyncResult<T> {
    type Item = T;
    type Error = Error;

    fn poll(&mut self) -> futures::Poll<Self::Item, Self::Error> {
        panic!()
    }
}
