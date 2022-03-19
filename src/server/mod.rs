use self::core::ServerCore;
use crate::auth::AuthParams;
use crate::transport::{
    ChannelDataTcpTransporter, ChannelDataUdpTransporter, StunTcpTransporter, StunTransporter,
    StunUdpTransporter,
};
use crate::turn_message::{TurnMessageDecoder, TurnMessageEncoder};
use crate::Error;
use factory::DefaultFactory;
use fibers::{BoxSpawn, Spawn};
use fibers_transport::{
    FixedPeerTransporter, RcTransporter, TcpListener, TcpTransport, UdpTransport, UdpTransporter,
};
use futures::{Async, Future, Poll, Stream};
use std::net::SocketAddr;

mod core;

#[derive(Debug)]
#[must_use = "future do nothing unless polled"]
pub struct UdpServer {
    core: ServerCore<StunUdpTransporter, ChannelDataUdpTransporter>,
}
impl UdpServer {
    pub fn start(
        bind_addr: SocketAddr,
        auth_params: AuthParams,
    ) -> impl Future<Item = Self, Error = Error> {
        UdpTransporter::bind(bind_addr)
            .map_err(|e| track!(Error::from(e)))
            .map(move |transporter| {
                let transporter = RcTransporter::new(transporter);
                let stun = StunUdpTransporter::new(StunTransporter::new(transporter.clone()));
                let channel_data = ChannelDataUdpTransporter::new(transporter);
                let core = ServerCore::new(stun, channel_data, auth_params);
                UdpServer { core }
            })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.core
            .stun_transporter_ref()
            .inner_ref()
            .with_inner_ref(|x| x.local_addr())
    }
}
impl Future for UdpServer {
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match track!(self.core.poll()) {
            Err(e) => {
                log::warn!("{}", e);
                Ok(Async::NotReady)
            }
            other => other,
        }
    }
}

#[derive(Debug)]
#[must_use = "future do nothing unless polled"]
pub struct TcpServer {
    listener: TcpListener<DefaultFactory<TurnMessageEncoder>, DefaultFactory<TurnMessageDecoder>>,
    spawner: BoxSpawn,
    auth_params: AuthParams,
}
impl TcpServer {
    pub fn start<S>(
        spawner: S,
        bind_addr: SocketAddr,
        auth_params: AuthParams,
    ) -> impl Future<Item = Self, Error = Error>
    where
        S: Spawn + Send + 'static,
    {
        TcpListener::listen(bind_addr)
            .map_err(|e| track!(Error::from(e)))
            .map(move |listener| TcpServer {
                listener,
                spawner: spawner.boxed(),
                auth_params,
            })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.listener.local_addr()
    }
}
impl Future for TcpServer {
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        while let Async::Ready(transporter) = track!(self.listener.poll())? {
            if let Some(transporter) = transporter {
                let peer = transporter.peer_addr();
                let transporter = RcTransporter::new(transporter);
                let stun = StunTcpTransporter::new(StunTransporter::new(transporter.clone()));
                let stun = FixedPeerTransporter::new(peer, (), stun);
                let channel_data = ChannelDataTcpTransporter::new(transporter);
                let channel_data = FixedPeerTransporter::new(peer, (), channel_data);
                let auth_params = self.auth_params.clone();
                self.spawner.spawn(
                    ServerCore::new(stun, channel_data, auth_params).map_err(|e| panic!("{}", e)),
                );
            } else {
                return Ok(Async::Ready(()));
            }
        }
        Ok(Async::NotReady)
    }
}
