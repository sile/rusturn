use crate::attribute::Attribute;
use crate::turn_message::TurnMessage;
use fibers_transport::{
    PollRecv, PollSend, RcTransporter, Result, TcpTransport, Transport, UdpTransport,
};
use futures::Async;
use std::net::SocketAddr;
use stun_codec::{DecodedMessage, Message};

#[derive(Debug)]
pub struct StunTransporter<T: Transport> {
    inner: RcTransporter<T>,
}
impl<T> StunTransporter<T>
where
    T: Transport<SendItem = TurnMessage, RecvItem = TurnMessage>,
{
    pub fn new(inner: RcTransporter<T>) -> Self {
        StunTransporter { inner }
    }

    pub fn with_inner_ref<F, U>(&self, f: F) -> U
    where
        F: FnOnce(&T) -> U,
    {
        self.inner.with_inner_ref(f)
    }
}
impl<T> Transport for StunTransporter<T>
where
    T: Transport<SendItem = TurnMessage, RecvItem = TurnMessage>,
{
    type PeerAddr = T::PeerAddr;
    type SendItem = Message<Attribute>;
    type RecvItem = DecodedMessage<Attribute>;

    fn start_send(&mut self, peer: Self::PeerAddr, item: Self::SendItem) -> Result<()> {
        track!(self.inner.start_send(peer, TurnMessage::Stun(item)))
    }

    fn poll_send(&mut self) -> PollSend {
        track!(self.inner.poll_send())
    }

    fn poll_recv(&mut self) -> PollRecv<(Self::PeerAddr, Self::RecvItem)> {
        let do_recv = track!(self
            .inner
            .with_peek_recv(|_peer, item| { !matches!(item, TurnMessage::ChannelData(_)) }))?;
        if do_recv == Some(true) {
            match track!(self.inner.poll_recv())? {
                Async::Ready(Some((peer, TurnMessage::Stun(item)))) => {
                    Ok(Async::Ready(Some((peer, Ok(item)))))
                }
                Async::Ready(Some((peer, TurnMessage::BrokenStun(item)))) => {
                    Ok(Async::Ready(Some((peer, Err(item)))))
                }
                Async::Ready(Some(_)) => unreachable!(),
                Async::Ready(None) => Ok(Async::Ready(None)),
                Async::NotReady => Ok(Async::NotReady),
            }
        } else {
            Ok(Async::NotReady)
        }
    }
}
impl<T> TcpTransport for StunTransporter<T>
where
    T: TcpTransport<SendItem = TurnMessage, RecvItem = TurnMessage>,
{
    fn peer_addr(&self) -> SocketAddr {
        self.inner.with_inner_ref(|x| x.peer_addr())
    }

    fn local_addr(&self) -> SocketAddr {
        self.inner.with_inner_ref(|x| x.local_addr())
    }
}
impl<T> UdpTransport for StunTransporter<T>
where
    T: UdpTransport<SendItem = TurnMessage, RecvItem = TurnMessage>,
{
    fn local_addr(&self) -> SocketAddr {
        self.inner.with_inner_ref(|x| x.local_addr())
    }
}
