use std::net::SocketAddr;
use std::collections::HashMap;
use slog::{self, Logger};
use futures::{self, Future, BoxFuture};
use rustun::{self, HandleMessage};
use rustun::message::Indication;
use rustun::rfc5389::attributes::XorMappedAddress;

use {Error, Method, Attribute};
use rfc5766::errors;

type Request = rustun::message::Request<Method, Attribute>;
type Response = rustun::message::Response<Method, Attribute>;

#[derive(Debug)]
pub struct DefaultHandler {
    logger: Logger,
    allocations: HashMap<SocketAddr, ()>,
}
impl DefaultHandler {
    pub fn new() -> Self {
        Self::with_logger(Logger::root(slog::Discard, o!()))
    }
    pub fn with_logger(logger: Logger) -> Self {
        DefaultHandler {
            logger: logger,
            allocations: HashMap::new(),
        }
    }

    fn handle_binding(&mut self, client: SocketAddr, request: Request) -> BoxFuture<Response, ()> {
        let mut response = request.into_success_response();
        response.add_attribute(XorMappedAddress::new(client));
        futures::finished(Ok(response)).boxed()
    }
    fn handle_allocate(&mut self, client: SocketAddr, request: Request) -> BoxFuture<Response, ()> {
        if self.allocations.contains_key(&client) {
            info!(self.logger, "Existing allocation: {}", client);
            let response = request.into_error_response()
                .with_error_code(errors::AllocationMismatch);
            return futures::finished(Err(response)).boxed();
        }
        panic!()
    }
}
impl HandleMessage for DefaultHandler {
    type Method = Method;
    type Attribute = Attribute;
    type HandleCall = BoxFuture<Response, ()>;
    type HandleCast = BoxFuture<(), ()>;
    fn handle_call(&mut self, client: SocketAddr, request: Request) -> Self::HandleCall {
        debug!(self.logger, "RECV: {:?}", request);
        match *request.method() {
            Method::Binding => self.handle_binding(client, request),
            Method::Allocate => self.handle_allocate(client, request),
            _ => unimplemented!(),
        }
    }
    fn handle_cast(&mut self,
                   _client: SocketAddr,
                   _message: Indication<Self::Method, Self::Attribute>)
                   -> Self::HandleCast {
        futures::finished(()).boxed()
    }
    fn handle_error(&mut self, client: SocketAddr, error: Error) {
        warn!(self.logger,
              "Cannot handle a message from the client {}: {}",
              client,
              error);
    }
}
