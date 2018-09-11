use futures::{Async, Future, Poll, Stream};
use rustun::channel::Channel as StunChannel;
use rustun::message::{MessageError, Request, Response};
use rustun::transport::{
    RetransmitTransporter, StunTransport, StunUdpTransporter, TcpTransporter, UdpTransporter,
};
use std::net::SocketAddr;
use stun_codec::rfc5766;
use stun_codec::{MessageDecoder, MessageEncoder};

use types::TransportProtocol;
use Error;

// TODO: move
pub type StunTcpTransporter<A> = TcpTransporter<MessageDecoder<A>, MessageEncoder<A>>;

pub type TurnUdpTransporter = StunUdpTransporter<rfc5766::Attribute>;
pub type TurnTcpTransporter = StunTcpTransporter<rfc5766::Attribute>;

#[derive(Debug)]
pub struct Client<T> {
    server_addr: SocketAddr,
    stun_channel: StunChannel<rfc5766::Attribute, T>,
}
impl<T> Client<T>
where
    T: StunTransport<rfc5766::Attribute>,
{
    pub fn new(server_addr: SocketAddr, transporter: T) -> Self {
        Client {
            server_addr,
            stun_channel: StunChannel::new(transporter),
        }
    }

    pub fn allocate(
        &mut self,
        username: &str,
        password: &str,
    ) -> impl Future<Item = Response<rfc5766::Attribute>, Error = MessageError> {
        let mut request = Request::new(rfc5766::methods::ALLOCATE);
        request.push_attribute(
            rfc5766::attributes::RequestedTransport::new(TransportProtocol::Udp.as_u8()).into(),
        );
        self.stun_channel.call(self.server_addr, request)
    }
}
impl Client<TurnUdpTransporter> {
    pub fn new_udp(server_addr: SocketAddr) -> impl Future<Item = Self, Error = Error> {
        let local_addr = "0.0.0.0:0".parse().expect("never fails");
        track_err!(UdpTransporter::bind(local_addr))
            .map(move |udp| Self::new(server_addr, RetransmitTransporter::new(udp)))
    }
}
impl Client<TurnTcpTransporter> {
    pub fn new_tcp(server_addr: SocketAddr) -> impl Future<Item = Self, Error = Error> {
        track_err!(TcpTransporter::connect(server_addr).map(move |tcp| Self::new(server_addr, tcp)))
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
