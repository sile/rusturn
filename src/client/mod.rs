use self::core::ClientCore;
use crate::auth::AuthParams;
use std::os::fd::AsRawFd;

use crate::transport::{
    ChannelDataTcpTransporter, ChannelDataUdpTransporter, StunTcpTransporter, StunTransporter,
    StunUdpTransporter,
};
use crate::{AsyncResult, Error, ErrorKind, Result};
use fibers_transport::{
    FixedPeerTransporter, RcTransporter, TcpTransport, TcpTransporter, UdpTransport, UdpTransporter,
};
use futures::{Async, Future, Poll};
use std::net::SocketAddr;

mod allocate;
mod core;
mod stun_transaction;

pub trait Client {
    fn create_permission(&mut self, peer: SocketAddr) -> AsyncResult<()>;
    fn channel_bind(&mut self, peer: SocketAddr) -> AsyncResult<()>;
    fn start_send(&mut self, peer: SocketAddr, data: Vec<u8>) -> Result<()>;
    fn poll_send(&mut self) -> Poll<(), Error>;
    fn poll_recv(&mut self) -> Poll<Option<(SocketAddr, Vec<u8>)>, Error>;
    fn local_addr(&self) -> SocketAddr;
    fn file_descriptor(&self) -> Option<i32>;
}

pub fn wait<C, FN, FU>(
    mut client: C,
    f: FN,
) -> impl Future<Item = (C, std::result::Result<FU::Item, FU::Error>), Error = Error>
where
    C: Client,
    FN: FnOnce(&mut C) -> FU,
    FU: Future,
{
    let future = f(&mut client);
    Wait {
        client: Some(client),
        future,
    }
}

#[derive(Debug)]
struct Wait<T, F> {
    client: Option<T>,
    future: F,
}
impl<T: Client, F: Future> Future for Wait<T, F> {
    type Item = (T, std::result::Result<F::Item, F::Error>);
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        track!(self
            .client
            .as_mut()
            .expect("Cannot Wait poll twice")
            .poll_send())?;
        if let Async::Ready(item) = track!(self
            .client
            .as_mut()
            .expect("Cannot Wait poll twice")
            .poll_recv())?
        {
            track_panic!(ErrorKind::Other, "Unexpected reception: {:?}", item);
        }
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
            })
            .map(TcpClient)
    }

    pub fn relay_addr(&self) -> Option<SocketAddr> {
        self.0.relay_addr()
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

    fn start_send(&mut self, peer: SocketAddr, data: Vec<u8>) -> Result<()> {
        self.0.start_send(peer, data)
    }

    fn poll_send(&mut self) -> Poll<(), Error> {
        self.0.poll_send()
    }

    fn poll_recv(&mut self) -> Poll<Option<(SocketAddr, Vec<u8>)>, Error> {
        self.0.poll_recv()
    }

    fn local_addr(&self) -> SocketAddr {
        self.0
            .stun_channel_ref()
            .transporter_ref()
            .inner_ref()
            .with_inner_ref(|x| x.local_addr())
    }
    fn file_descriptor(&self) -> Option<i32> {
        None
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
            })
            .map(UdpClient)
    }

    pub fn relay_addr(&self) -> Option<SocketAddr> {
        self.0.relay_addr()
    }
}
unsafe impl Send for UdpClient {}
impl Client for UdpClient {
    fn file_descriptor(&self) -> Option<i32> {
        let mut my_fd: Option<i32> = None;
        self.0
            .stun_channel_ref()
            .transporter_ref()
            .inner_ref()
            .inner_ref()
            .with_inner_ref(|f| {
                let socket: &fibers::net::UdpSocket = f.socket_ref();
                socket.with_inner(|g| my_fd = Some(g.as_raw_fd()));
            });
        my_fd
    }

    fn create_permission(&mut self, peer: SocketAddr) -> AsyncResult<()> {
        self.0.create_permission(peer)
    }

    fn channel_bind(&mut self, peer: SocketAddr) -> AsyncResult<()> {
        self.0.channel_bind(peer)
    }

    fn start_send(&mut self, peer: SocketAddr, data: Vec<u8>) -> Result<()> {
        self.0.start_send(peer, data)
    }

    fn poll_send(&mut self) -> Poll<(), Error> {
        self.0.poll_send()
    }

    fn poll_recv(&mut self) -> Poll<Option<(SocketAddr, Vec<u8>)>, Error> {
        self.0.poll_recv()
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
