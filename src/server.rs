use fibers::net::futures::{RecvFrom, SendTo};
use fibers::net::UdpSocket;
use fibers::sync::mpsc;
use fibers::sync::oneshot;
use fibers::{BoxSpawn, Spawn};
use futures::{self, Async, Future, Poll, Stream};
use rand;
use rustun::message::Message;
use rustun::rfc5389;
use rustun::rfc5389::attributes::{MessageIntegrity, Nonce, Realm, Username, XorMappedAddress};
use rustun::rfc5389::attributes::{Software, UnknownAttributes};
use rustun::server::IndicationSender;
use rustun::{self, HandleMessage, Method as StunMethod};
use slog::{self, Logger};
use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use rfc5766;
use rfc5766::attributes::{DontFragment, EvenPort, RequestedTransport, ReservationToken};
use rfc5766::attributes::{Lifetime, XorPeerAddress, XorRelayedAddress};
use rfc5766::errors;
use {Attribute, BoxFuture, Error, Method};

type Request = rustun::message::Request<Method, Attribute>;
type Indication = rustun::message::Indication<Method, Attribute>;
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
    pub fn with_logger<S: Spawn + Send + 'static>(
        logger: Logger,
        spawner: S,
        addr: IpAddr,
    ) -> Self {
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
        //let chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        let chars = "abcdefghijklmnopqrstuvwxyz0123456789";
        let mut buf = String::new();
        for _ in 0..16 {
            let i = rand::random::<usize>() % chars.len();
            buf.push(chars.as_bytes()[i] as char);
        }
        Nonce::new(buf).unwrap()
    }
    fn handle_binding(&mut self, client: SocketAddr, request: Request) -> BoxFuture<Response, ()> {
        let mut response = request.into_success_response();
        response.add_attribute(XorMappedAddress::new(client));
        Box::new(futures::finished(Ok(response)))
    }

    fn check_credential(
        &self,
        client: SocketAddr,
        request: Request,
    ) -> Result<Request, ErrorResponse> {
        if let Some(message_integrity) = request.get_attribute::<MessageIntegrity>().cloned() {
            let username = if let Some(a) = request.get_attribute::<Username>().cloned() {
                a
            } else {
                warn!(self.logger, "'{}' has no 'USERNAME' attribute", client);
                let response = request
                    .into_error_response()
                    .with_error_code(rfc5389::errors::BadRequest);
                return Err(response);
            };
            let realm = if let Some(a) = request.get_attribute::<Realm>().cloned() {
                a
            } else {
                warn!(self.logger, "'{}' has no 'REALM' attribute", client);
                let response = request
                    .into_error_response()
                    .with_error_code(rfc5389::errors::BadRequest);
                return Err(response);
            };
            let nonce = if let Some(a) = request.get_attribute::<Nonce>().cloned() {
                a
            } else {
                warn!(self.logger, "'{}' has no 'NONCE' attribute", client);
                let response = request
                    .into_error_response()
                    .with_error_code(rfc5389::errors::BadRequest);
                return Err(response);
            };
            // TODO: "stale nonce" check

            info!(
                self.logger,
                "'{}': username={}, realm={}, nonce={}",
                client,
                username.name(),
                realm.text(),
                nonce.value()
            );
            if let Err(e) =
                message_integrity.check_long_term_credential(&username, &realm, &self.password)
            {
                warn!(
                    self.logger,
                    "Message integrity check for '{}' is failed: {}", client, e
                );
                let mut response = request
                    .into_error_response()
                    .with_error_code(rfc5389::errors::Unauthorized);
                response.add_attribute(self.realm.clone());
                let nonce = self.random_nonce();
                info!(self.logger, "New NONCE for '{}': {:?}", client, nonce);
                response.add_attribute(nonce);
                return Err(response);
            }
            info!(
                self.logger,
                "'{}' has the valid 'MESSAGE-INTEGRITY' attribute", client
            );
            Ok(request)
        } else {
            info!(
                self.logger,
                "'{}' has no 'MESSAGE-INTEGRITY' attribute", client
            );
            let mut response = request
                .into_error_response()
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
                return Box::new(futures::finished(Err(response)));
            }
            Ok(request) => request,
        };
        let username = request.get_attribute::<Username>().cloned().unwrap();
        let realm = request.get_attribute::<Realm>().cloned().unwrap();

        // 2.
        if self.allocations.contains_key(&client) {
            info!(self.logger, "Existing allocation: {}", client);
            let response = request
                .into_error_response()
                .with_error_code(errors::AllocationMismatch);
            return Box::new(futures::finished(Err(response)));
        }

        // 3.
        match request.get_attribute::<RequestedTransport>().cloned() {
            None => {
                let response = request
                    .into_error_response()
                    .with_error_code(rfc5389::errors::BadRequest);
                return Box::new(futures::finished(Err(response)));
            }
            Some(a) if !a.is_udp() => {
                let response = request
                    .into_error_response()
                    .with_error_code(errors::UnsupportedTransportProtocol);
                return Box::new(futures::finished(Err(response)));
            }
            _ => {}
        }

        // 4.
        if let Some(a) = request.get_attribute::<DontFragment>().cloned() {
            use rustun::Attribute;
            info!(
                self.logger,
                "This server does not support 'DONT-FRAGMENT' attribute"
            );
            let mut response = request
                .into_error_response()
                .with_error_code(rfc5389::errors::UnknownAttribute);
            response.add_attribute(UnknownAttributes::new(vec![a.get_type()]));
            return Box::new(futures::finished(Err(response)));
        }

        // 5.
        if let Some(a) = request.get_attribute::<ReservationToken>().cloned() {
            use rustun::Attribute;
            info!(
                self.logger,
                "This server does not support 'RESERVATION-TOKEN' attribute"
            );
            let mut response = request
                .into_error_response()
                .with_error_code(rfc5389::errors::UnknownAttribute);
            response.add_attribute(UnknownAttributes::new(vec![a.get_type()]));
            return Box::new(futures::finished(Err(response)));
        }

        // 6.
        if let Some(a) = request.get_attribute::<EvenPort>().cloned() {
            use rustun::Attribute;
            info!(
                self.logger,
                "This server does not support 'EVEN-PORT' attribute"
            );
            let mut response = request
                .into_error_response()
                .with_error_code(rfc5389::errors::UnknownAttribute);
            response.add_attribute(UnknownAttributes::new(vec![a.get_type()]));
            return Box::new(futures::finished(Err(response)));
        }

        // 7.

        // 8.

        //
        let logger = self.logger.clone();
        let info_tx = self.info_tx.clone();
        let (tx, rx) = oneshot::channel();
        let password = self.password.clone();
        let relay_addr = format!("{}:0", self.addr).parse().unwrap();
        let (forward_tx, forward_rx) = mpsc::channel();
        let future = track_err!(UdpSocket::bind(relay_addr))
            .map_err(|e: Error| panic!("Error: {}", e))
            .and_then(move |socket| {
                let relayed_addr = socket.local_addr().unwrap();
                info!(
                    logger,
                    "Creates the allocation for '{}' (relayed_addr='{}')", client, relayed_addr
                );

                let mut response = request.into_success_response();
                response.add_attribute(XorRelayedAddress::new(relayed_addr));
                response.add_attribute(XorMappedAddress::new(client));
                response.add_attribute(Lifetime::new(Duration::from_secs(600)));
                response.add_attribute(Software::new("None".to_string()).unwrap());
                let mi = MessageIntegrity::new_long_term_credential(
                    &response, &username, &realm, &password,
                ).unwrap();
                response.add_attribute(mi);
                tx.send(Ok(response)).unwrap();
                RelayLoop::new(logger, info_tx, forward_rx, client, socket)
            });
        self.spawner.spawn(future);
        self.allocations
            .insert(client, Allocation::new(self.logger.clone(), forward_tx));
        Box::new(rx.map_err(|_| ()))
    }
    fn handle_refresh(&mut self, client: SocketAddr, request: Request) -> BoxFuture<Response, ()> {
        let request = match self.check_credential(client, request) {
            Err(response) => {
                return Box::new(futures::finished(Err(response)));
            }
            Ok(request) => request,
        };
        let username = request.get_attribute::<Username>().cloned().unwrap();
        let realm = request.get_attribute::<Realm>().cloned().unwrap();

        let lifetime = request
            .get_attribute::<Lifetime>()
            .cloned()
            .unwrap_or_else(|| Lifetime::new(Duration::from_secs(600)));
        if lifetime.duration().as_secs() == 0 {
            info!(self.logger, "Remove the allocation for '{}'", client);
        }

        let mut response = request.into_success_response();
        response.add_attribute(Lifetime::new(lifetime.duration()));
        response.add_attribute(Software::new("None".to_string()).unwrap());
        let mi = MessageIntegrity::new_long_term_credential(
            &response,
            &username,
            &realm,
            &self.password,
        ).unwrap();
        response.add_attribute(mi);
        Box::new(futures::finished(Ok(response)))
    }
    fn handle_create_permission(
        &mut self,
        client: SocketAddr,
        request: Request,
    ) -> BoxFuture<Response, ()> {
        let request = match self.check_credential(client, request) {
            Err(response) => {
                return Box::new(futures::finished(Err(response)));
            }
            Ok(request) => request,
        };
        let username = request.get_attribute::<Username>().cloned().unwrap();
        let realm = request.get_attribute::<Realm>().cloned().unwrap();

        // TODO: Support multiple permissions
        let peer = if let Some(a) = request.get_attribute::<XorPeerAddress>().cloned() {
            a
        } else {
            warn!(
                self.logger,
                "'{}' has no 'XOR-PEER-ADDRESS' attribute", client
            );
            let response = request
                .into_error_response()
                .with_error_code(rfc5389::errors::BadRequest);
            return Box::new(futures::finished(Err(response)));
        };
        info!(
            self.logger,
            "New permission installed for '{}': permitted peer is '{}'",
            client,
            peer.address()
        );

        // TODO: install permission

        let mut response = request.into_success_response();
        response.add_attribute(Software::new("None".to_string()).unwrap());
        let mi = MessageIntegrity::new_long_term_credential(
            &response,
            &username,
            &realm,
            &self.password,
        ).unwrap();
        response.add_attribute(mi);
        Box::new(futures::finished(Ok(response)))
    }
    fn handle_data(&mut self, client: SocketAddr, peer: SocketAddr, data: Vec<u8>) {
        // debug!(self.logger,
        //        "Relays {} bytes data from '{}' to '{}'",
        //        data.len(),
        //        peer,
        //        client);
        let mut indication = rfc5766::methods::Data.indication::<Attribute>();
        indication.add_attribute(rfc5766::attributes::Data::new(data));
        indication.add_attribute(XorPeerAddress::new(peer));
        self.indication_tx
            .as_mut()
            .unwrap()
            .send(client, indication)
            .unwrap();
    }
    fn handle_send(&mut self, client: SocketAddr, indication: Indication) -> BoxFuture<(), ()> {
        // TODO: error handlings
        let peer = indication.get_attribute::<XorPeerAddress>().unwrap();
        assert!(indication.get_attribute::<DontFragment>().is_none());
        let data = indication
            .get_attribute::<rfc5766::attributes::Data>()
            .unwrap();
        if let Some(allocation) = self.allocations.get(&client) {
            allocation
                .forward_tx
                .send((peer.address(), data.clone().unwrap()))
                .unwrap();
        }
        Box::new(futures::finished(()))
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
        match *request.method() {
            Method::Binding => self.handle_binding(client, request),
            Method::Allocate => self.handle_allocate(client, request),
            Method::CreatePermission => self.handle_create_permission(client, request),
            Method::Refresh => self.handle_refresh(client, request),
            _ => {
                warn!(self.logger, "RECV({}): {:?}", client, request);
                unimplemented!()
            }
        }
    }
    fn handle_cast(&mut self, client: SocketAddr, indication: Indication) -> Self::HandleCast {
        match *indication.method() {
            Method::Send => self.handle_send(client, indication),
            _ => {
                warn!(self.logger, "RECV({}): {:?}", client, indication);
                unimplemented!()
            }
        }
    }
    fn handle_error(&mut self, client: SocketAddr, error: Error) {
        warn!(
            self.logger,
            "Cannot handle a message from the client '{}': {}", client, error
        );
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
    forward_tx: mpsc::Sender<(SocketAddr, Vec<u8>)>,
}
impl Allocation {
    pub fn new(logger: Logger, forward_tx: mpsc::Sender<(SocketAddr, Vec<u8>)>) -> Self {
        Allocation {
            logger: logger,
            permissions: Vec::new(),
            channels: Vec::new(),
            forward_tx: forward_tx,
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
    forward_rx: mpsc::Receiver<(SocketAddr, Vec<u8>)>,
    send: Option<SendTo<Vec<u8>>>,
    queue: VecDeque<(SocketAddr, Vec<u8>)>,
}
impl RelayLoop {
    fn new(
        logger: Logger,
        info_tx: mpsc::Sender<Info>,
        forward_rx: mpsc::Receiver<(SocketAddr, Vec<u8>)>,
        client: SocketAddr,
        socket: UdpSocket,
    ) -> Self {
        info!(
            logger,
            "Starts relay loop for '{}' (relayed = '{}')",
            client,
            socket.local_addr().unwrap()
        );
        RelayLoop {
            logger: logger,
            info_tx: info_tx,
            forward_rx: forward_rx,
            client: client,
            socket: socket.clone(),
            recv: socket.recv_from(vec![0; 10240]),
            send: None,
            queue: VecDeque::new(),
        }
    }
}
impl Future for RelayLoop {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            match self.forward_rx.poll().unwrap() {
                Async::NotReady => break,
                Async::Ready(None) => return Ok(Async::Ready(())),
                Async::Ready(Some(item)) => self.queue.push_back(item),
            }
        }
        loop {
            match self.send.poll() {
                Err(e) => {
                    warn!(self.logger, "Socket send error: {:?}", e);
                    return Err(());
                }
                Ok(Async::NotReady) => break,
                Ok(Async::Ready(_)) => {
                    self.send = None;
                    if let Some((peer, data)) = self.queue.pop_front() {
                        debug!(
                            self.logger,
                            "RELAY: '{}' ='{}'=> '{}' ({} bytes)",
                            self.client,
                            self.socket.local_addr().unwrap(),
                            peer,
                            data.len()
                        );
                        self.send = Some(self.socket.clone().send_to(data, peer));
                    } else {
                        break;
                    }
                }
            }
        }
        match track_err!(self.recv.poll().map_err(|(_, _, e)| e)) {
            Err(e) => {
                let e: Error = e;
                warn!(self.logger, "Datagram receiving error: {}", e);
                return Err(());
            }
            Ok(Async::NotReady) => {}
            Ok(Async::Ready((socket, mut buf, size, peer))) => {
                buf.truncate(size);
                debug!(
                    self.logger,
                    "RELAY: '{}' <='{}'= '{} ({} bytes)",
                    self.client,
                    self.socket.local_addr().unwrap(),
                    peer,
                    buf.len()
                );
                self.info_tx
                    .send(Info::Data(self.client, peer, buf))
                    .unwrap();
                self.recv = socket.recv_from(vec![0; 10240]);
            }
        }
        Ok(Async::NotReady)
    }
}
