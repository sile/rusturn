#[macro_use]
extern crate slog;
extern crate fibers;
extern crate futures;
extern crate rand;
extern crate rustun;
#[macro_use]
extern crate trackable;
extern crate handy_async;

macro_rules! track_try {
    ($expr:expr) => {
        track!($expr.map_err(::Error::from))?
    };
}
macro_rules! track_err {
    ($expr:expr) => {
        $expr.map_err(|e| track!(::Error::from(e)))
    };
}

pub use attribute::Attribute;
pub use method::Method;
pub use rustun::{Error, ErrorKind};

pub mod rfc5766;
pub mod server;
pub mod types;

mod attribute;
mod method;

pub type Result<T> = ::std::result::Result<T, Error>;

type BoxFuture<T, E> = Box<futures::Future<Item = T, Error = E> + Send + 'static>;
