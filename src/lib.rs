#[macro_use]
extern crate slog;
extern crate rand;
extern crate rustun;
extern crate fibers;
extern crate futures;
#[macro_use]
extern crate trackable;
extern crate handy_async;

macro_rules! track_try {
    ($expr:expr) => {
        track!($expr.map_err(::Error::from))?
    }
}
macro_rules! track_err {
    ($expr:expr) => {
        $expr.map_err(|e| track!(::Error::from(e)))
    }
}

pub use rustun::{Error, ErrorKind};
pub use method::Method;
pub use attribute::Attribute;

pub mod types;
pub mod rfc5766;
pub mod server;

mod method;
mod attribute;

pub type Result<T> = ::std::result::Result<T, Error>;

type BoxFuture<T, E> = Box<futures::Future<Item = T, Error = E> + Send + 'static>;
