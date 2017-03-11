#[macro_use]
extern crate slog;
extern crate rand;
extern crate rustun;
extern crate fibers;
extern crate futures;
#[macro_use]
extern crate trackable;
extern crate handy_async;

pub use rustun::{Error, ErrorKind};
pub use method::Method;
pub use attribute::Attribute;

pub mod types;
pub mod rfc5766;
pub mod server;

mod method;
mod attribute;

pub type Result<T> = ::std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
