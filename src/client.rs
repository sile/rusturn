use futures::{self, Async, Future, Poll, Stream};
use rustun::channel::Channel as StunChannel;
use rustun::message::{MessageError, Request, Response, SuccessResponse};
use rustun::transport::{
    RetransmitTransporter, StunTransport, StunUdpTransporter, TcpTransporter, UdpTransporter,
};
use std::net::SocketAddr;
use std::time::Duration;
use stun_codec::{rfc5389, rfc5766};
use stun_codec::{MessageDecoder, MessageEncoder};

use types::TransportProtocol;
use {Error, ErrorKind, Result};

// TODO: move
pub type StunTcpTransporter<A> = TcpTransporter<MessageDecoder<A>, MessageEncoder<A>>;

pub type TurnUdpTransporter = StunUdpTransporter<rfc5766::Attribute>;
pub type TurnTcpTransporter = StunTcpTransporter<rfc5766::Attribute>;

#[derive(Debug)]
pub struct Client<T> {
    server_addr: SocketAddr,
    stun_channel: StunChannel<rfc5766::Attribute, T>,
    lifetime: Duration,
}
impl<T> Client<T> where T: StunTransport<rfc5766::Attribute> {}
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
    T: StunTransport<rfc5766::Attribute>,
{
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Error> {
        while let Async::Ready(message) = track!(self.stun_channel.poll())? {
            if let Some(message) = message {
                panic!("TODO: {:?}", message);
            } else {
                return Ok(Async::Ready(None));
            }
        }
        Ok(Async::NotReady)
    }
}

struct Allocate<T> {
    server_addr: SocketAddr,
    stun_channel: Option<StunChannel<rfc5766::Attribute, T>>,
    username: String,
    password: String,
    realm: Option<String>,
    nonce: Option<String>,
    future: Box<Future<Item = Response<rfc5766::Attribute>, Error = MessageError> + Send + 'static>,
}
impl<T> Allocate<T>
where
    T: StunTransport<rfc5766::Attribute> + Send + 'static,
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
        request.push_attribute(requested_transport);

        if let Some(realm) = self.realm.clone() {
            let nonce = self.nonce.clone().take().expect("never fails");
            let username = rfc5389::attributes::Username::new(self.username.clone()).expect("TODO");
            let realm = rfc5389::attributes::Realm::new(realm).expect("TODO");
            let nonce = rfc5389::attributes::Nonce::new(nonce).expect("TOOD");
            request.push_attribute(username.clone().into());
            request.push_attribute(realm.clone().into());
            request.push_attribute(nonce.clone().into());
            let mi = rfc5389::attributes::MessageIntegrity::new_long_term_credential(
                request.clone().into_message(),
                &username,
                &realm,
                &self.password,
            ).expect("TODO");
            request.push_attribute(mi.into());
        }
        self.future = Box::new(
            self.stun_channel
                .as_mut()
                .expect("never fails")
                .call(self.server_addr, request),
        );
    }

    fn make_client(&mut self, response: SuccessResponse<rfc5766::Attribute>) -> Result<Client<T>> {
        let mut lifetime = None;
        for attr in response.attributes() {
            match attr {
                rfc5766::Attribute::Lifetime(a) => {
                    lifetime = Some(a.lifetime());
                }
                rfc5766::Attribute::MessageIntegrity(a) => {
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
        })
    }
}
impl<T> Future for Allocate<T>
where
    T: StunTransport<rfc5766::Attribute> + Send + 'static,
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
                                rfc5766::Attribute::ErrorCode(e) => {
                                    track_assert_eq!(e.code(),
                                                     rfc5389::errors::Unauthorized::CODEPOINT,
                                                     ErrorKind::Other; response);
                                }
                                rfc5766::Attribute::Realm(a) => {
                                    self.realm = Some(a.text().to_owned());
                                }
                                rfc5766::Attribute::Nonce(a) => {
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
