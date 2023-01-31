use crate::channel_data::ChannelData;
use crate::turn_message::TurnMessage;
use fibers_transport::{PollRecv, PollSend, RcTransporter, Result, Transport};
use futures::Async;

#[derive(Debug)]
pub struct ChannelDataTransporter<T: Transport> {
    inner: RcTransporter<T>,
}
impl<T> ChannelDataTransporter<T>
where
    T: Transport<SendItem = TurnMessage, RecvItem = TurnMessage>,
{
    pub fn new(inner: RcTransporter<T>) -> Self {
        ChannelDataTransporter { inner }
    }
}
impl<T> Transport for ChannelDataTransporter<T>
where
    T: Transport<SendItem = TurnMessage, RecvItem = TurnMessage>,
{
    type PeerAddr = T::PeerAddr;
    type SendItem = ChannelData;
    type RecvItem = ChannelData;

    fn start_send(&mut self, peer: Self::PeerAddr, item: Self::SendItem) -> Result<()> {
        track!(self.inner.start_send(peer, TurnMessage::ChannelData(item)))
    }

    fn poll_send(&mut self) -> PollSend {
        track!(self.inner.poll_send())
    }

    fn poll_recv(&mut self) -> PollRecv<(Self::PeerAddr, Self::RecvItem)> {
        let do_recv = track!(self
            .inner
            .with_peek_recv(|_peer, item| { matches!(item, TurnMessage::ChannelData(_)) }))?;
        if do_recv == Some(true) {
            match track!(self.inner.poll_recv())? {
                Async::Ready(Some((peer, TurnMessage::ChannelData(item)))) => {
                    Ok(Async::Ready(Some((peer, item))))
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
