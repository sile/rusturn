#[macro_use]
extern crate slog;
extern crate rustun;
extern crate fibers;
extern crate futures;
#[macro_use]
extern crate trackable;

pub use error::{Error, ErrorKind};

mod error;

pub type Result<T> = ::std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
