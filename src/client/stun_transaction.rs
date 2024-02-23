use crate::attribute::Attribute;
use futures::{Future, Poll};
use rustun::message::{MessageError, Response};
use std::fmt;
use std::net::SocketAddr;

pub struct StunTransaction<T = Response<Attribute>>(
    Box<dyn Future<Item = T, Error = MessageError> + Send + 'static>,
);
impl StunTransaction<Response<Attribute>> {
    pub fn new<F>(future: F) -> Self
    where
        F: Future<Item = Response<Attribute>, Error = MessageError> + Send + 'static,
    {
        StunTransaction(Box::new(future.fuse()))
    }
}
impl StunTransaction<(SocketAddr, Response<Attribute>)> {
    pub fn with_peer<F>(peer: SocketAddr, future: F) -> Self
    where
        F: Future<Item = Response<Attribute>, Error = MessageError> + Send + 'static,
    {
        StunTransaction(Box::new(future.map(move |item| (peer, item)).fuse()))
    }
}
impl<T> StunTransaction<T>
where
    T: Send + 'static,
{
    pub fn empty() -> Self {
        StunTransaction(Box::new(futures::empty()))
    }
}
impl<T> Future for StunTransaction<T> {
    type Item = T;
    type Error = MessageError;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.0.poll()
    }
}
impl<T> fmt::Debug for StunTransaction<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "StunTransaction(_)")
    }
}
