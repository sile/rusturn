use fibers_transport::{
    FixedPeerTransporter, RcTransporter, TcpTransport, TcpTransporter, UdpTransport, UdpTransporter,
};
use futures::{self, Async, Future, Poll};
use rustun::message::{MessageError, Response};
use std;
use std::fmt;
use std::net::SocketAddr;

use self::core::ClientCore;
use attribute::Attribute;
use auth::AuthParams;
use transport::{
    ChannelDataTcpTransporter, ChannelDataUdpTransporter, StunTcpTransporter, StunTransporter,
    StunUdpTransporter,
};
use {AsyncResult, Error, Result};

mod allocate;
mod core;

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

pub trait Client {
    fn create_permission(&mut self, peer: SocketAddr) -> AsyncResult<()>;
    fn channel_bind(&mut self, peer: SocketAddr) -> AsyncResult<()>;
    fn send_data(&mut self, peer: SocketAddr, data: Vec<u8>) -> Result<()>;
    fn send_channel_data(&mut self, peer: SocketAddr, data: Vec<u8>) -> Result<()>;
    fn recv_data(&mut self) -> Option<(SocketAddr, Vec<u8>)>;

    // TODO: poll_xxx
    fn run_once(&mut self) -> Result<()>;
    fn local_addr(&self) -> SocketAddr;

    fn wait<FN, FUT>(mut self, f: FN) -> Wait<Self, FUT>
    where
        Self: Sized,
        FN: FnOnce(&mut Self) -> FUT,
        FUT: Future,
    {
        let future = f(&mut self);
        Wait {
            client: Some(self),
            future,
        }
    }
}

#[derive(Debug)]
pub struct Wait<T, F> {
    client: Option<T>,
    future: F,
}
impl<T: Client, F: Future> Future for Wait<T, F> {
    type Item = (T, std::result::Result<F::Item, F::Error>);
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        track!(
            self.client
                .as_mut()
                .expect("Cannot Wait poll twice")
                .run_once()
        )?;
        match self.future.poll() {
            Err(e) => Ok(Async::Ready((
                self.client.take().expect("never fails"),
                Err(e),
            ))),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Ok(Async::Ready(v)) => Ok(Async::Ready((
                self.client.take().expect("never fails"),
                Ok(v),
            ))),
        }
    }
}

#[derive(Debug)]
pub struct TcpClient(ClientCore<StunTcpTransporter, ChannelDataTcpTransporter>);
impl TcpClient {
    pub fn allocate(
        server_addr: SocketAddr,
        auth_params: AuthParams,
    ) -> impl Future<Item = Self, Error = Error> {
        TcpTransporter::connect(server_addr)
            .map_err(|e| track!(Error::from(e)))
            .and_then(move |transporter| {
                let transporter = RcTransporter::new(transporter);
                let stun = StunTcpTransporter::new(StunTransporter::new(transporter.clone()));
                let channel_data = ChannelDataTcpTransporter::new(transporter);
                track_err!(ClientCore::allocate(stun, channel_data, auth_params))
            }).map(TcpClient)
    }
}
unsafe impl Send for TcpClient {}
impl Client for TcpClient {
    fn create_permission(&mut self, peer: SocketAddr) -> AsyncResult<()> {
        self.0.create_permission(peer)
    }

    fn channel_bind(&mut self, peer: SocketAddr) -> AsyncResult<()> {
        self.0.channel_bind(peer)
    }

    fn send_data(&mut self, peer: SocketAddr, data: Vec<u8>) -> Result<()> {
        self.0.send_data(peer, data)
    }

    fn send_channel_data(&mut self, peer: SocketAddr, data: Vec<u8>) -> Result<()> {
        self.0.send_channel_data(peer, data)
    }

    fn recv_data(&mut self) -> Option<(SocketAddr, Vec<u8>)> {
        self.0.recv_data()
    }

    fn run_once(&mut self) -> Result<()> {
        self.0.run_once()
    }

    fn local_addr(&self) -> SocketAddr {
        self.0
            .stun_channel_ref()
            .transporter_ref()
            .inner_ref()
            .with_inner_ref(|x| x.local_addr())
    }
}

#[derive(Debug)]
pub struct UdpClient(
    ClientCore<
        FixedPeerTransporter<StunUdpTransporter, ()>,
        FixedPeerTransporter<ChannelDataUdpTransporter, ()>,
    >,
);
impl UdpClient {
    pub fn allocate(
        server_addr: SocketAddr,
        auth_params: AuthParams,
    ) -> impl Future<Item = Self, Error = Error> {
        let bind_addr = "0.0.0.0:0".parse().expect("never fails");

        UdpTransporter::bind(bind_addr)
            .map_err(|e| track!(Error::from(e)))
            .and_then(move |transporter| {
                let transporter = RcTransporter::new(transporter);
                let stun = StunUdpTransporter::new(StunTransporter::new(transporter.clone()));
                let stun = FixedPeerTransporter::new((), server_addr, stun);
                let channel_data = ChannelDataUdpTransporter::new(transporter);
                let channel_data = FixedPeerTransporter::new((), server_addr, channel_data);
                track_err!(ClientCore::allocate(stun, channel_data, auth_params))
            }).map(UdpClient)
    }
}
unsafe impl Send for UdpClient {}
impl Client for UdpClient {
    fn create_permission(&mut self, peer: SocketAddr) -> AsyncResult<()> {
        self.0.create_permission(peer)
    }

    fn channel_bind(&mut self, peer: SocketAddr) -> AsyncResult<()> {
        self.0.channel_bind(peer)
    }

    fn send_data(&mut self, peer: SocketAddr, data: Vec<u8>) -> Result<()> {
        self.0.send_data(peer, data)
    }

    fn send_channel_data(&mut self, peer: SocketAddr, data: Vec<u8>) -> Result<()> {
        self.0.send_channel_data(peer, data)
    }

    fn recv_data(&mut self) -> Option<(SocketAddr, Vec<u8>)> {
        self.0.recv_data()
    }

    fn run_once(&mut self) -> Result<()> {
        self.0.run_once()
    }

    fn local_addr(&self) -> SocketAddr {
        self.0
            .stun_channel_ref()
            .transporter_ref()
            .inner_ref()
            .inner_ref()
            .with_inner_ref(|x| x.local_addr())
    }
}
