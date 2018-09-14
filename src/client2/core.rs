use futures::{Async, Future, Poll};
use rustun::channel::Channel as StunChannel;
use rustun::transport::{StunTransport, Transport};
use std::net::SocketAddr;
use std::time::Duration;

use super::allocate::Allocate;
use super::Client;
use attribute::Attribute;
use auth::AuthParams;
use channel_data::{ChannelData, ChannelNumber};
use {AsyncResult, Error, Result};

#[derive(Debug)]
pub struct ClientCore<S, C>
where
    S: StunTransport<Attribute>,
    C: Transport<SendItem = ChannelData, RecvItem = ChannelData>,
{
    server_addr: SocketAddr,
    stun_channel: StunChannel<Attribute, S>,
    channel_data_transporter: C,
    auth_params: AuthParams,
    lifetime: Duration,
}
impl<S, C> ClientCore<S, C>
where
    S: StunTransport<Attribute> + 'static,
    C: Transport<SendItem = ChannelData, RecvItem = ChannelData>,
{
    pub fn allocate(
        stun_transporter: S,
        channel_data_transporter: C,
        server_addr: SocketAddr,
        auth_params: AuthParams,
    ) -> Allocate<S, C> {
        Allocate::new(
            server_addr,
            StunChannel::new(stun_transporter),
            channel_data_transporter,
            auth_params,
        )
    }

    pub fn new(
        server_addr: SocketAddr,
        stun_channel: StunChannel<Attribute, S>,
        channel_data_transporter: C,
        auth_params: AuthParams,
        lifetime: Duration,
    ) -> Self {
        ClientCore {
            server_addr,
            stun_channel,
            channel_data_transporter,
            auth_params,
            lifetime,
        }
    }
}
impl<S, C> Client for ClientCore<S, C>
where
    S: StunTransport<Attribute> + 'static,
    C: Transport<SendItem = ChannelData, RecvItem = ChannelData>,
{
    fn create_permission(&mut self, peer: SocketAddr) -> AsyncResult<()> {
        panic!()
    }

    fn channel_bind(&mut self, peer: SocketAddr) -> AsyncResult<ChannelNumber> {
        panic!()
    }

    fn send_data(&mut self, peer: SocketAddr, data: Vec<u8>) -> Result<()> {
        panic!()
    }

    fn send_channel_data(&mut self, channel_number: ChannelNumber, data: Vec<u8>) -> Result<()> {
        panic!()
    }

    fn recv_data(&mut self) -> Option<(SocketAddr, Vec<u8>)> {
        panic!()
    }

    fn run_once(&mut self) -> Result<()> {
        panic!()
    }
}
