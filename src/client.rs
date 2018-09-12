use fibers::time::timer::{self, Timeout};
use futures::{self, Async, Future, Poll, Stream};
use rustun::channel::Channel as StunChannel;
use rustun::message::{Indication, MessageError, Request, Response, SuccessResponse};
use rustun::transport::{
    RetransmitTransporter, StunTcpTransporter, StunTransport, StunUdpTransporter, TcpTransporter,
    UdpTransporter,
};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, SystemTime};
use stun_codec::{rfc5389, rfc5766};

use attribute::Attribute;
use types::TransportProtocol;
use {Error, ErrorKind, Result};

pub const PERMISSION_LIFETIME_SECONDS: u64 = 300;

pub type TurnUdpTransporter = StunUdpTransporter<Attribute>;
pub type TurnTcpTransporter = StunTcpTransporter<Attribute>;

type ResponseFuture =
    Box<dyn Future<Item = Response<Attribute>, Error = MessageError> + Send + 'static>;

#[derive(Debug)]
pub enum PermissionState {
    Creating,

    // TODO: delete (UDPのタイミング逆転を考えると云々)
    Installed { time_to_expiry: SystemTime },
}

// #[derive(Debug)]
pub struct Client<T>
where
    T: StunTransport<Attribute> + Send + 'static,
{
    server_addr: SocketAddr,
    stun_channel: StunChannel<Attribute, T>,
    lifetime: Duration,
    refresh_transaction: ResponseFuture,
    refresh_timout: Option<Timeout>,
    create_permission_transaction: ResponseFuture,
    channel_bind_transaction: ResponseFuture,
    username: String,
    password: String,
    realm: String,
    nonce: String,
    permissions: HashMap<IpAddr, PermissionState>,
}
impl<T> Client<T>
where
    T: StunTransport<Attribute> + Send + 'static,
{
    pub fn refresh(&mut self, lifetime: Option<Duration>) -> Result<()> {
        let mut request = Request::new(rfc5766::methods::REFRESH);

        let lifetime = lifetime.unwrap_or(self.lifetime);
        request.add_attribute(track!(rfc5766::attributes::Lifetime::new(lifetime))?.into());

        let username = track!(rfc5389::attributes::Username::new(self.username.clone()))?;
        let realm = track!(rfc5389::attributes::Realm::new(self.realm.clone()))?;
        let nonce = track!(rfc5389::attributes::Nonce::new(self.nonce.clone()))?;
        request.add_attribute(username.clone().into());
        request.add_attribute(realm.clone().into());
        request.add_attribute(nonce.clone().into());
        let mi = rfc5389::attributes::MessageIntegrity::new_long_term_credential(
            request.as_ref(),
            &username,
            &realm,
            &self.password,
        ).expect("TODO");
        request.add_attribute(mi.into());

        self.refresh_transaction = Box::new(self.stun_channel.call(self.server_addr, request));
        Ok(())
    }

    // TODO: return future(?)
    // TODO: return `Transport`
    pub fn create_permission(&mut self, peer: SocketAddr) -> Result<()> {
        if self.permissions.contains_key(&peer.ip()) {
            return Ok(());
        }

        let mut request = Request::new(rfc5766::methods::CREATE_PERMISSION);

        request.add_attribute(rfc5766::attributes::XorPeerAddress::new(peer).into());

        let username = track!(rfc5389::attributes::Username::new(self.username.clone()))?;
        let realm = track!(rfc5389::attributes::Realm::new(self.realm.clone()))?;
        let nonce = track!(rfc5389::attributes::Nonce::new(self.nonce.clone()))?;
        request.add_attribute(username.clone().into());
        request.add_attribute(realm.clone().into());
        request.add_attribute(nonce.clone().into());
        let mi = rfc5389::attributes::MessageIntegrity::new_long_term_credential(
            request.as_ref(),
            &username,
            &realm,
            &self.password,
        ).expect("TODO");
        request.add_attribute(mi.into());

        self.create_permission_transaction =
            Box::new(self.stun_channel.call(self.server_addr, request));
        self.permissions
            .insert(peer.ip(), PermissionState::Creating);
        Ok(())
    }

    pub fn channel_bind(&mut self, peer: SocketAddr) -> Result<()> {
        if self.permissions.contains_key(&peer.ip()) {
            return Ok(());
        }

        let mut request = Request::new(rfc5766::methods::CHANNEL_BIND);

        request.add_attribute(rfc5766::attributes::XorPeerAddress::new(peer).into());

        // TODO:
        request.add_attribute(
            rfc5766::attributes::ChannelNumber::new(super::channel_data::MIN_CHANNEL_NUMBER).into(),
        );

        let username = track!(rfc5389::attributes::Username::new(self.username.clone()))?;
        let realm = track!(rfc5389::attributes::Realm::new(self.realm.clone()))?;
        let nonce = track!(rfc5389::attributes::Nonce::new(self.nonce.clone()))?;
        request.add_attribute(username.clone().into());
        request.add_attribute(realm.clone().into());
        request.add_attribute(nonce.clone().into());
        let mi = track!(
            rfc5389::attributes::MessageIntegrity::new_long_term_credential(
                request.as_ref(),
                &username,
                &realm,
                &self.password,
            )
        )?;
        request.add_attribute(mi.into());

        self.channel_bind_transaction = Box::new(self.stun_channel.call(self.server_addr, request));
        self.permissions
            .insert(peer.ip(), PermissionState::Creating);
        Ok(())
    }

    pub fn send(&mut self, peer: SocketAddr, data: Vec<u8>) -> Result<()> {
        track_assert!(self.permissions.contains_key(&peer.ip()), ErrorKind::Other); // TODO

        let mut indication = Indication::new(rfc5766::methods::SEND);
        indication.add_attribute(rfc5766::attributes::XorPeerAddress::new(peer).into());
        indication.add_attribute(track!(rfc5766::attributes::Data::new(data))?.into());
        self.stun_channel.cast(self.server_addr, indication);

        Ok(())
    }

    // TODO:
    pub fn channel_send(&mut self, channel_number: u16, data: Vec<u8>) -> Result<()> {
        // TODO: check channels
        panic!()
    }

    // pub fn wait<F>(
    //     self,
    //     future: F,
    // ) -> impl Future<Item = (Self, std::result::Result<F::Item, F::Error>), Error = Error>
    // where
    //     F: Future,
    // {
    // }
}
impl Client<TurnUdpTransporter> {
    pub fn udp_allocate(
        server_addr: SocketAddr,
        username: String,
        password: String,
    ) -> impl Future<Item = Self, Error = Error> {
        let local_addr = "0.0.0.0:0".parse().expect("never fails");
        track_err!(UdpTransporter::bind(local_addr)).and_then(move |udp| {
            Allocate::new(
                server_addr,
                RetransmitTransporter::new(udp),
                username,
                password,
            )
        })
    }
}
impl Client<TurnTcpTransporter> {
    pub fn tcp_allocate(
        server_addr: SocketAddr,
        username: String,
        password: String,
    ) -> impl Future<Item = Self, Error = Error> {
        track_err!(TcpTransporter::connect(server_addr))
            .and_then(move |tcp| Allocate::new(server_addr, tcp, username, password))
    }
}
impl<T> Stream for Client<T>
where
    T: StunTransport<Attribute> + Send + 'static,
{
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Error> {
        let mut did_something = true;

        while did_something {
            did_something = false;

            while let Async::Ready(message) = track!(self.stun_channel.poll())? {
                did_something = true;
                if let Some(message) = message {
                    println!("TODO: {:?}", message);
                } else {
                    return Ok(Async::Ready(None));
                }
            }
            if let Async::Ready(response) = track!(self.refresh_transaction.poll())? {
                did_something = true;
                self.refresh_transaction = Box::new(futures::empty());

                // TODO: check lifetime and nonce, update timeout (and integrity)
                println!("# TODO: {:?}", response);
            }
            if let Async::Ready(response) = track!(self.create_permission_transaction.poll())? {
                did_something = true;
                self.create_permission_transaction = Box::new(futures::empty());

                // TODO: check lifetime and nonce, update timeout (and integrity)
                println!("# TODO: {:?}", response);
            }
            if let Async::Ready(response) = track!(self.channel_bind_transaction.poll())? {
                did_something = true;
                self.channel_bind_transaction = Box::new(futures::empty());

                // TODO: check lifetime and nonce, update timeout (and integrity)
                println!("# TODO: {:?}", response);
            }
            if let Async::Ready(Some(())) = self.refresh_timout.poll().expect("TODO") {
                did_something = true;
                self.refresh_timout = None;

                track!(self.refresh(None))?;
            }
        }

        Ok(Async::NotReady)
    }
}
impl<T> Drop for Client<T>
where
    T: StunTransport<Attribute> + Send + 'static,
{
    fn drop(&mut self) {
        let _ = self.refresh(Some(Duration::from_secs(0)));
        let _ = self.stun_channel.poll();
    }
}

struct Allocate<T> {
    server_addr: SocketAddr,
    stun_channel: Option<StunChannel<Attribute, T>>,
    username: String,
    password: String,
    realm: Option<String>,
    nonce: Option<String>,
    future: ResponseFuture,
}
impl<T> Allocate<T>
where
    T: StunTransport<Attribute> + Send + 'static,
{
    fn new(server_addr: SocketAddr, transporter: T, username: String, password: String) -> Self {
        let stun_channel = StunChannel::new(transporter);
        let mut this = Allocate {
            server_addr,
            stun_channel: Some(stun_channel),
            username,
            password,
            realm: None,
            nonce: None,
            future: Box::new(futures::empty()),
        };
        this.allocate();
        this
    }

    fn allocate(&mut self) {
        let mut request = Request::new(rfc5766::methods::ALLOCATE);

        let requested_transport =
            rfc5766::attributes::RequestedTransport::new(TransportProtocol::Udp.as_u8()).into();
        request.add_attribute(requested_transport);

        if let Some(realm) = self.realm.clone() {
            let nonce = self.nonce.clone().take().expect("never fails");
            let username = rfc5389::attributes::Username::new(self.username.clone()).expect("TODO");
            let realm = rfc5389::attributes::Realm::new(realm).expect("TODO");
            let nonce = rfc5389::attributes::Nonce::new(nonce).expect("TOOD");
            request.add_attribute(username.clone().into());
            request.add_attribute(realm.clone().into());
            request.add_attribute(nonce.clone().into());
            let mi = rfc5389::attributes::MessageIntegrity::new_long_term_credential(
                request.as_ref(),
                &username,
                &realm,
                &self.password,
            ).expect("TODO");
            request.add_attribute(mi.into());
        }
        self.future = Box::new(
            self.stun_channel
                .as_mut()
                .expect("never fails")
                .call(self.server_addr, request),
        );
    }

    fn make_client(&mut self, response: SuccessResponse<Attribute>) -> Result<Client<T>> {
        let mut lifetime = None;
        for attr in response.attributes() {
            match attr {
                Attribute::Lifetime(a) => {
                    lifetime = Some(a.lifetime());
                }
                Attribute::MessageIntegrity(a) => {
                    let realm = track_assert_some!(self.realm.as_ref(), ErrorKind::Other);
                    // TODO: track!(..)?;
                    track_assert!(
                        a.check_long_term_credential(
                            &rfc5389::attributes::Username::new(self.username.clone())
                                .expect("TODO"),
                            &rfc5389::attributes::Realm::new(realm.to_owned()).expect("TODO"),
                            &self.password
                        ).is_ok(),
                        ErrorKind::Other
                    );
                }
                _ => {}
            }
        }

        let lifetime = track_assert_some!(lifetime, ErrorKind::Other);
        Ok(Client {
            server_addr: self.server_addr,
            stun_channel: self.stun_channel.take().expect("never fails"),
            lifetime,
            username: self.username.clone(),
            password: self.password.clone(),
            realm: self.realm.clone().take().expect("never fails"),
            nonce: self.nonce.clone().take().expect("never fails"),
            refresh_transaction: Box::new(futures::empty()),
            refresh_timout: Some(timer::timeout(lifetime * 10 / 9)),
            permissions: HashMap::new(),
            create_permission_transaction: Box::new(futures::empty()),
            channel_bind_transaction: Box::new(futures::empty()),
        })
    }
}
impl<T> Future for Allocate<T>
where
    T: StunTransport<Attribute> + Send + 'static,
{
    type Item = Client<T>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let mut did_something = true;
        while did_something {
            did_something = false;

            while let Async::Ready(message) = track!(
                self.stun_channel
                    .as_mut()
                    .expect("Cannot poll Allocate twice")
                    .poll()
            )? {
                did_something = true;
                if let Some(message) = message {
                    println!("# TODO(queuing): {:?}", message);
                } else {
                    track_panic!(ErrorKind::Other);
                }
            }
            if let Async::Ready(response) = track!(self.future.poll())? {
                did_something = true;
                match response {
                    Ok(response) => {
                        let client = track!(self.make_client(response))?;
                        return Ok(Async::Ready(client));
                    }
                    Err(response) => {
                        track_assert!(self.realm.is_none(), ErrorKind::Other; response);
                        for attr in response.attributes() {
                            match attr {
                                Attribute::ErrorCode(e) => {
                                    track_assert_eq!(e.code(),
                                                     rfc5389::errors::Unauthorized::CODEPOINT,
                                                     ErrorKind::Other; response);
                                }
                                Attribute::Realm(a) => {
                                    self.realm = Some(a.text().to_owned());
                                }
                                Attribute::Nonce(a) => {
                                    self.nonce = Some(a.value().to_owned());
                                }
                                _ => {}
                            }
                        }
                        track_assert!(self.realm.is_some(), ErrorKind::Other; response);
                        track_assert!(self.nonce.is_some(), ErrorKind::Other; response);
                        self.allocate();
                    }
                }
            }
        }
        Ok(Async::NotReady)
    }
}
