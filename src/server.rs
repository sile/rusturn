use std::time::Duration;
use std::net::{SocketAddr, IpAddr};
use std::collections::HashMap;
use rand;
use slog::{self, Logger};
use futures::{self, Future, BoxFuture};
use rustun::{self, HandleMessage};
use rustun::message::{Message, Indication};
use rustun::rfc5389;
use rustun::rfc5389::attributes::{XorMappedAddress, MessageIntegrity, Username, Nonce, Realm};
use rustun::rfc5389::attributes::UnknownAttributes;

use {Error, Method, Attribute};
use rfc5766::errors;
use rfc5766::attributes::{RequestedTransport, DontFragment, ReservationToken, EvenPort};
use rfc5766::attributes::{Lifetime, XorRelayedAddress};

type Request = rustun::message::Request<Method, Attribute>;
type Response = rustun::message::Response<Method, Attribute>;
type ErrorResponse = rustun::message::ErrorResponse<Method, Attribute>;

#[derive(Debug)]
pub struct DefaultHandler {
    logger: Logger,
    addr: IpAddr,
    realm: Realm,
    password: String,
    allocations: HashMap<SocketAddr, Allocation>,
}
impl DefaultHandler {
    pub fn new(addr: IpAddr) -> Self {
        Self::with_logger(Logger::root(slog::Discard, o!()), addr)
    }
    pub fn with_logger(logger: Logger, addr: IpAddr) -> Self {
        DefaultHandler {
            logger: logger,
            addr: addr,
            realm: rfc5389::attributes::Realm::new("localhost".to_string()).unwrap(),
            password: "foobarbaz".to_string(), // XXX
            allocations: HashMap::new(),
        }
    }
    pub fn set_realm(&mut self, realm: Realm) {
        self.realm = realm;
    }
    pub fn set_password(&mut self, password: &str) {
        self.password = password.to_string();
    }

    fn random_nonce(&self) -> Nonce {
        let chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        let mut buf = String::new();
        for _ in 0..32 {
            let i = rand::random::<usize>() % chars.len();
            buf.push(chars.as_bytes()[i] as char);
        }
        Nonce::new(buf).unwrap()
    }
    fn handle_binding(&mut self, client: SocketAddr, request: Request) -> BoxFuture<Response, ()> {
        let mut response = request.into_success_response();
        response.add_attribute(XorMappedAddress::new(client));
        futures::finished(Ok(response)).boxed()
    }

    fn check_credential(&self,
                        client: SocketAddr,
                        request: Request)
                        -> Result<Request, ErrorResponse> {
        if let Some(message_integrity) = request.get_attribute::<MessageIntegrity>().cloned() {
            let username = if let Some(a) = request.get_attribute::<Username>().cloned() {
                a
            } else {
                warn!(self.logger, "'{}' has no 'USERNAME' attribute", client);
                let response = request.into_error_response()
                    .with_error_code(rfc5389::errors::BadRequest);
                return Err(response);
            };
            let realm = if let Some(a) = request.get_attribute::<Realm>().cloned() {
                a
            } else {
                warn!(self.logger, "'{}' has no 'REALM' attribute", client);
                let response = request.into_error_response()
                    .with_error_code(rfc5389::errors::BadRequest);
                return Err(response);
            };
            let nonce = if let Some(a) = request.get_attribute::<Nonce>().cloned() {
                a
            } else {
                warn!(self.logger, "'{}' has no 'NONCE' attribute", client);
                let response = request.into_error_response()
                    .with_error_code(rfc5389::errors::BadRequest);
                return Err(response);
            };
            // TODO: "stale nonce" check

            info!(self.logger,
                  "'{}': username={}, realm={}, nonce={}",
                  client,
                  username.name(),
                  realm.text(),
                  nonce.value());
            if let Err(e) =
                message_integrity.check_long_term_credential(&username, &realm, &self.password) {
                warn!(self.logger,
                      "Message integrity check for '{}' is failed: {}",
                      client,
                      e);
                let mut response = request.into_error_response()
                    .with_error_code(rfc5389::errors::Unauthorized);
                response.add_attribute(self.realm.clone());
                let nonce = self.random_nonce();
                info!(self.logger, "New NONCE for '{}': {:?}", client, nonce);
                response.add_attribute(nonce);
                return Err(response);
            }
            info!(self.logger,
                  "'{}' has the valid 'MESSAGE-INTEGRITY' attribute",
                  client);
            Ok(request)
        } else {
            info!(self.logger,
                  "'{}' has no 'MESSAGE-INTEGRITY' attribute",
                  client);
            let mut response = request.into_error_response()
                .with_error_code(rfc5389::errors::Unauthorized);
            response.add_attribute(self.realm.clone());
            let nonce = self.random_nonce();
            info!(self.logger, "NONCE for '{}': {:?}", client, nonce);
            response.add_attribute(nonce);
            Err(response)
        }
    }
    fn handle_allocate(&mut self, client: SocketAddr, request: Request) -> BoxFuture<Response, ()> {
        // 6.2.  Receiving an Allocate Request
        //
        // https://tools.ietf.org/html/rfc5766#section-6.2

        info!(self.logger, "Allocation request from '{}'", client);

        // 1.
        let request = match self.check_credential(client, request) {
            Err(response) => {
                return futures::finished(Err(response)).boxed();
            }
            Ok(request) => request,
        };
        let username = request.get_attribute::<Username>().cloned().unwrap();
        let realm = request.get_attribute::<Realm>().cloned().unwrap();

        // 2.
        if self.allocations.contains_key(&client) {
            info!(self.logger, "Existing allocation: {}", client);
            let response = request.into_error_response()
                .with_error_code(errors::AllocationMismatch);
            return futures::finished(Err(response)).boxed();
        }

        // 3.
        match request.get_attribute::<RequestedTransport>().cloned() {
            None => {
                let response = request.into_error_response()
                    .with_error_code(rfc5389::errors::BadRequest);
                return futures::finished(Err(response)).boxed();
            }
            Some(a) if !a.is_udp() => {
                let response = request.into_error_response()
                    .with_error_code(errors::UnsupportedTransportProtocol);
                return futures::finished(Err(response)).boxed();
            }
            _ => {}
        }

        // 4.
        if let Some(a) = request.get_attribute::<DontFragment>().cloned() {
            use rustun::Attribute;
            info!(self.logger,
                  "This server does not support 'DONT-FRAGMENT' attribute");
            let mut response = request.into_error_response()
                .with_error_code(rfc5389::errors::UnknownAttribute);
            response.add_attribute(UnknownAttributes::new(vec![a.get_type()]));
            return futures::finished(Err(response)).boxed();
        }

        // 5.
        if let Some(a) = request.get_attribute::<ReservationToken>().cloned() {
            use rustun::Attribute;
            info!(self.logger,
                  "This server does not support 'RESERVATION-TOKEN' attribute");
            let mut response = request.into_error_response()
                .with_error_code(rfc5389::errors::UnknownAttribute);
            response.add_attribute(UnknownAttributes::new(vec![a.get_type()]));
            return futures::finished(Err(response)).boxed();
        }

        // 6.
        if let Some(a) = request.get_attribute::<EvenPort>().cloned() {
            use rustun::Attribute;
            info!(self.logger,
                  "This server does not support 'EVEN-PORT' attribute");
            let mut response = request.into_error_response()
                .with_error_code(rfc5389::errors::UnknownAttribute);
            response.add_attribute(UnknownAttributes::new(vec![a.get_type()]));
            return futures::finished(Err(response)).boxed();
        }

        // 7.

        // 8.
        let relay_port = client.port() + 7; // TODO
        let relayed_addr = SocketAddr::new(self.addr, relay_port);

        info!(self.logger,
              "Creates the allocation for '{}' (relayed_addr='{}'",
              client,
              relayed_addr);

        self.allocations.insert(client, Allocation::new(self.logger.clone()));
        let mut response = request.into_success_response();
        response.add_attribute(XorRelayedAddress::new(relayed_addr));
        response.add_attribute(XorMappedAddress::new(client));
        response.add_attribute(Lifetime::new(Duration::from_secs(600)));
        let mi = MessageIntegrity::new_long_term_credential(&response,
                                                            &username,
                                                            &realm,
                                                            &self.password)
                .unwrap();
        response.add_attribute(mi);
        futures::finished(Ok(response)).boxed()
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

#[derive(Debug)]
pub struct Allocation {
    logger: Logger,
    permissions: Vec<()>,
    channels: Vec<()>,
}
impl Allocation {
    pub fn new(logger: Logger) -> Self {
        Allocation {
            logger: logger,
            permissions: Vec::new(),
            channels: Vec::new(),
        }
    }
}
