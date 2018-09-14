#[macro_use]
extern crate bytecodec;
extern crate fibers;
extern crate fibers_timeout_queue;
extern crate futures;
extern crate rand;
extern crate rustun;
#[macro_use]
extern crate stun_codec;
#[macro_use]
extern crate trackable;

pub use rustun::{Error, ErrorKind, Result};

pub mod attribute;
pub mod auth;
pub mod channel_data;
pub mod client;
pub mod transport;

pub const DEFAULT_LIFETIME_SECONDS: u64 = 10 * 60;

#[derive(Debug)]
pub struct AsyncResult<T>(fibers::sync::oneshot::Monitor<T, Error>);
impl<T> AsyncResult<T> {
    fn new() -> (Self, AsyncReply<T>) {
        let (tx, rx) = fibers::sync::oneshot::monitor();
        (AsyncResult(rx), AsyncReply(tx))
    }
}
impl<T> futures::Future for AsyncResult<T> {
    type Item = T;
    type Error = Error;

    fn poll(&mut self) -> futures::Poll<Self::Item, Self::Error> {
        track!(self.0.poll().map_err(Error::from))
    }
}

#[derive(Debug)]
struct AsyncReply<T>(fibers::sync::oneshot::Monitored<T, Error>);
impl<T> AsyncReply<T> {
    fn send(self, result: Result<T>) {
        let _ = self.0.exit(result);
    }
}
