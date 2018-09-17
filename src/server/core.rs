use fibers_transport::Transport;
use futures::{Async, Future, Poll};
use rustun::channel::Channel as StunChannel;
use rustun::channel::RecvMessage;
use rustun::transport::StunTransport;
use std::collections::HashMap;
use std::net::SocketAddr;
use stun_codec::TransactionId;

use attribute::Attribute;
use channel_data::ChannelData;
use Error;

#[derive(Debug)]
pub struct ServerCore<S, C>
where
    S: StunTransport<Attribute, PeerAddr = SocketAddr>,
    C: Transport<PeerAddr = SocketAddr, SendItem = ChannelData, RecvItem = ChannelData>,
{
    stun_channel: StunChannel<Attribute, S>,
    channel_data_transporter: C,
    allocations: HashMap<SocketAddr, AllocationState>,
    seqnum: u64,
}
impl<S, C> ServerCore<S, C>
where
    S: StunTransport<Attribute, PeerAddr = SocketAddr>,
    C: Transport<PeerAddr = SocketAddr, SendItem = ChannelData, RecvItem = ChannelData>,
{
    pub fn new(stun_transporter: S, channel_data_transporter: C) -> Self {
        ServerCore {
            stun_channel: StunChannel::new(stun_transporter),
            channel_data_transporter,
            allocations: HashMap::new(),
            seqnum: 0,
        }
    }

    pub fn stun_transporter_ref(&self) -> &S {
        self.stun_channel.transporter_ref()
    }

    fn handle_stun_message(&mut self, peer: SocketAddr, message: RecvMessage<Attribute>) {
        panic!()
    }

    fn handle_channel_data(&mut self, peer: SocketAddr, data: ChannelData) {
        panic!()
    }
}
unsafe impl<S, C> Send for ServerCore<S, C>
where
    S: StunTransport<Attribute, PeerAddr = SocketAddr>,
    C: Transport<PeerAddr = SocketAddr, SendItem = ChannelData, RecvItem = ChannelData>,
{}
impl<S, C> Future for ServerCore<S, C>
where
    S: StunTransport<Attribute, PeerAddr = SocketAddr>,
    C: Transport<PeerAddr = SocketAddr, SendItem = ChannelData, RecvItem = ChannelData>,
{
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let mut did_something = true;
        while did_something {
            did_something = false;

            while let Async::Ready(message) = track!(self.stun_channel.poll_recv())? {
                did_something = true;
                if let Some((peer, message)) = message {
                    self.handle_stun_message(peer, message);
                } else {
                    return Ok(Async::Ready(()));
                }
            }
            while let Async::Ready(data) = track!(self.channel_data_transporter.poll_recv())? {
                did_something = true;
                if let Some((peer, data)) = data {
                    self.handle_channel_data(peer, data);
                } else {
                    return Ok(Async::Ready(()));
                }
            }
            track!(self.stun_channel.poll_send())?;
            track!(self.channel_data_transporter.poll_send())?;
        }
        Ok(Async::NotReady)
    }
}

#[derive(Debug)]
struct AllocationState {
    transaction_id: TransactionId,
    seqnum: u64,
}
