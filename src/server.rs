use std::time::Duration;
use std::net::{SocketAddr, IpAddr};
use std::collections::HashMap;
use rand;
use slog::{self, Logger};
use fibers::{Spawn, BoxSpawn};
use fibers::net::UdpSocket;
use fibers::net::futures::RecvFrom;
use fibers::sync::mpsc;
use fibers::sync::oneshot;
use futures::{self, Future, BoxFuture, Poll, Async};
use rustun::{self, HandleMessage, Method as StunMethod};
use rustun::server::IndicationSender;
use rustun::message::{Message, Indication};
use rustun::rfc5389;
use rustun::rfc5389::attributes::{XorMappedAddress, MessageIntegrity, Username, Nonce, Realm};
use rustun::rfc5389::attributes::UnknownAttributes;

use {Error, Method, Attribute};
use rfc5766;
use rfc5766::errors;
use rfc5766::attributes::{RequestedTransport, DontFragment, ReservationToken, EvenPort};
use rfc5766::attributes::{Lifetime, XorRelayedAddress, XorPeerAddress};

type Request = rustun::message::Request<Method, Attribute>;
type Response = rustun::message::Response<Method, Attribute>;
type ErrorResponse = rustun::message::ErrorResponse<Method, Attribute>;

#[derive(Debug)]
pub enum Info {
    Data(SocketAddr, SocketAddr, Vec<u8>),
}

#[derive(Debug)]
pub struct DefaultHandler {
    logger: Logger,
    spawner: BoxSpawn,
    addr: IpAddr,
    realm: Realm,
    password: String,
    allocations: HashMap<SocketAddr, Allocation>,
    info_tx: mpsc::Sender<Info>,
    indication_tx: Option<IndicationSender>,
}
impl DefaultHandler {
    pub fn new<S: Spawn + Send + 'static>(spawner: S, addr: IpAddr) -> Self {
        Self::with_logger(Logger::root(slog::Discard, o!()), spawner, addr)
    }
    pub fn with_logger<S: Spawn + Send + 'static>(logger: Logger,
                                                  spawner: S,
                                                  addr: IpAddr)
                                                  -> Self {
        let (info_tx, _) = mpsc::channel();
        DefaultHandler {
            logger: logger,
            spawner: spawner.boxed(),
            addr: addr,
            realm: rfc5389::attributes::Realm::new("localhost".to_string()).unwrap(),
            password: "foobarbaz".to_string(), // XXX
            allocations: HashMap::new(),
            info_tx: info_tx, // NOTE: dummy initial value
            indication_tx: None,
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

        //
        let logger = self.logger.clone();
        let info_tx = self.info_tx.clone();
        let (tx, rx) = oneshot::channel();
        let password = self.password.clone();
        let relay_addr = format!("{}:0", self.addr).parse().unwrap();
        let future = track_err!(UdpSocket::bind(relay_addr))
            .map_err(|e: Error| panic!("Error: {}", e))
            .and_then(move |socket| {
                let relayed_addr = socket.local_addr().unwrap();
                info!(logger,
                      "Creates the allocation for '{}' (relayed_addr='{}'",
                      client,
                      relayed_addr);

                let mut response = request.into_success_response();
                response.add_attribute(XorRelayedAddress::new(relayed_addr));
                response.add_attribute(XorMappedAddress::new(client));
                response.add_attribute(Lifetime::new(Duration::from_secs(600)));
                let mi = MessageIntegrity::new_long_term_credential(&response,
                                                                    &username,
                                                                    &realm,
                                                                    &password)
                        .unwrap();
                response.add_attribute(mi);
                tx.send(Ok(response)).unwrap();
                RelayLoop::new(logger, info_tx, client, socket)
            });
        self.spawner.spawn(future);
        self.allocations.insert(client, Allocation::new(self.logger.clone()));
        rx.map_err(|_| ()).boxed()
    }
    fn handle_refresh(&mut self, client: SocketAddr, request: Request) -> BoxFuture<Response, ()> {
        let request = match self.check_credential(client, request) {
            Err(response) => {
                return futures::finished(Err(response)).boxed();
            }
            Ok(request) => request,
        };
        let username = request.get_attribute::<Username>().cloned().unwrap();
        let realm = request.get_attribute::<Realm>().cloned().unwrap();

        let lifetime = request.get_attribute::<Lifetime>()
            .cloned()
            .unwrap_or_else(|| Lifetime::new(Duration::from_secs(600)));
        if lifetime.duration().as_secs() == 0 {
            info!(self.logger, "Remove the allocation for '{}'", client);
        }

        let mut response = request.into_success_response();
        response.add_attribute(Lifetime::new(lifetime.duration()));
        let mi = MessageIntegrity::new_long_term_credential(&response,
                                                            &username,
                                                            &realm,
                                                            &self.password)
                .unwrap();
        response.add_attribute(mi);
        futures::finished(Ok(response)).boxed()
    }
    fn handle_create_permission(&mut self,
                                client: SocketAddr,
                                request: Request)
                                -> BoxFuture<Response, ()> {
        let request = match self.check_credential(client, request) {
            Err(response) => {
                return futures::finished(Err(response)).boxed();
            }
            Ok(request) => request,
        };
        let username = request.get_attribute::<Username>().cloned().unwrap();
        let realm = request.get_attribute::<Realm>().cloned().unwrap();

        // TODO: Support multiple permissions
        let peer = if let Some(a) = request.get_attribute::<XorPeerAddress>().cloned() {
            a
        } else {
            warn!(self.logger,
                  "'{}' has no 'XOR-PEER-ADDRESS' attribute",
                  client);
            let response = request.into_error_response()
                .with_error_code(rfc5389::errors::BadRequest);
            return futures::finished(Err(response)).boxed();
        };
        info!(self.logger,
              "New permission installed for '{}': permitted peer is '{}'",
              client,
              peer.address());

        // TODO: install permission

        let mut response = request.into_success_response();
        let mi = MessageIntegrity::new_long_term_credential(&response,
                                                            &username,
                                                            &realm,
                                                            &self.password)
                .unwrap();
        response.add_attribute(mi);
        futures::finished(Ok(response)).boxed()
    }
    fn handle_data(&mut self, client: SocketAddr, peer: SocketAddr, data: Vec<u8>) {
        debug!(self.logger,
               "Relays {} bytes data from '{}' to '{}'",
               data.len(),
               peer,
               client);
        let mut indication = rfc5766::methods::Data.indication::<Attribute>();
        indication.add_attribute(rfc5766::attributes::Data::new(data));
        indication.add_attribute(XorPeerAddress::new(peer));
        self.indication_tx
            .as_mut()
            .unwrap()
            .send(peer, indication)
            .unwrap();
    }
}
impl HandleMessage for DefaultHandler {
    type Method = Method;
    type Attribute = Attribute;
    type HandleCall = BoxFuture<Response, ()>;
    type HandleCast = BoxFuture<(), ()>;
    type Info = Info;
    fn on_init(&mut self, info_tx: mpsc::Sender<Info>, indication_tx: IndicationSender) {
        self.info_tx = info_tx;
        self.indication_tx = Some(indication_tx);
    }
    fn handle_call(&mut self, client: SocketAddr, request: Request) -> Self::HandleCall {
        debug!(self.logger, "RECV: {:?}", request);
        match *request.method() {
            Method::Binding => self.handle_binding(client, request),
            Method::Allocate => self.handle_allocate(client, request),
            Method::CreatePermission => self.handle_create_permission(client, request),
            Method::Refresh => self.handle_refresh(client, request),
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
    fn handle_info(&mut self, info: Info) {
        match info {
            Info::Data(client, peer, data) => self.handle_data(client, peer, data),
        }
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

#[derive(Debug)]
pub struct RelayLoop {
    logger: Logger,
    client: SocketAddr,
    info_tx: mpsc::Sender<Info>,
    socket: UdpSocket,
    recv: RecvFrom<Vec<u8>>,
}
impl RelayLoop {
    fn new(logger: Logger,
           info_tx: mpsc::Sender<Info>,
           client: SocketAddr,
           socket: UdpSocket)
           -> Self {
        RelayLoop {
            logger: logger,
            info_tx: info_tx,
            client: client,
            socket: socket.clone(),
            recv: socket.recv_from(vec![0; 1024]),
        }
    }
}
impl Future for RelayLoop {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match track_err!(self.recv.poll().map_err(|(_, _, e)| e)) {
            Err(e) => {
                let e: Error = e;
                warn!(self.logger, "Datagram receiving error: {}", e);
                return Err(());
            }
            Ok(Async::NotReady) => {}
            Ok(Async::Ready((socket, mut buf, size, peer))) => {
                buf.truncate(size);
                debug!(self.logger,
                       "Recv {} bytes from '{}', relays to '{}'",
                       buf.len(),
                       peer,
                       self.client);
                self.info_tx.send(Info::Data(self.client, peer, buf)).unwrap();
                self.recv = socket.recv_from(vec![0; 1024]);
            }
        }
        Ok(Async::NotReady)
    }
}
