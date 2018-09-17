use bytecodec::{Decode, DecodeExt, Encode, EncodeExt};
use fibers_transport::{ErrorKind, PollRecv, PollSend, Result, Transport, UdpTransport};
use futures::Async;
use std::collections::HashSet;
use std::net::SocketAddr;
use trackable::error::ErrorKindExt;

use client::Client;

#[derive(Debug)]
pub struct UdpOverTurnTransporter<C, E, D> {
    client: C,
    encoder: E,
    decoder: D,
    channels: HashSet<SocketAddr>,
}
impl<C, E, D> UdpOverTurnTransporter<C, E, D>
where
    C: Client,
    E: Encode + Default,
    D: Decode + Default,
{
    pub fn new(client: C) -> Self {
        UdpOverTurnTransporter {
            client,
            encoder: E::default(),
            decoder: D::default(),
            channels: HashSet::new(),
        }
    }

    fn ensure_channel_exists(&mut self, peer: SocketAddr) {
        if self.channels.insert(peer) {
            self.client.channel_bind(peer);
        }
    }
}
impl<C, E, D> Transport for UdpOverTurnTransporter<C, E, D>
where
    C: Client,
    E: Encode + Default,
    D: Decode + Default,
{
    type PeerAddr = SocketAddr;
    type SendItem = E::Item;
    type RecvItem = D::Item;

    fn start_send(&mut self, peer: SocketAddr, message: Self::SendItem) -> Result<()> {
        self.ensure_channel_exists(peer);
        let data = track!(self.encoder.encode_into_bytes(message))?;
        track!(
            self.client
                .send_channel_data(peer, data)
                .map_err(|e| ErrorKind::Other.takes_over(e))
        )?;
        Ok(())
    }

    fn poll_send(&mut self) -> PollSend {
        track!(
            self.client
                .run_once()
                .map_err(|e| ErrorKind::Other.takes_over(e))
        )?;
        Ok(Async::Ready(())) // TODO
    }

    fn poll_recv(&mut self) -> PollRecv<(SocketAddr, Self::RecvItem)> {
        if let Some((peer, data)) = self.client.recv_data() {
            let item = track!(self.decoder.decode_from_bytes(&data))?;
            Ok(Async::Ready(Some((peer, item))))
        } else {
            Ok(Async::NotReady)
        }
    }
}
impl<C, E, D> UdpTransport for UdpOverTurnTransporter<C, E, D>
where
    C: Client,
    E: Encode + Default,
    D: Decode + Default,
{
    fn local_addr(&self) -> SocketAddr {
        self.client.local_addr()
    }
}
