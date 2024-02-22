use super::allocate::Allocate;
use super::stun_transaction::StunTransaction;
use crate::attribute::Attribute;
use crate::auth::AuthParams;
use crate::channel_data::ChannelData;
use crate::{AsyncReply, AsyncResult, Error, ErrorKind, Result};
use fibers_timeout_queue::TimeoutQueue;
use fibers_transport::Transport;
use futures::{Async, Future, Poll};
use rustun::channel::{Channel as StunChannel, RecvMessage};
use rustun::message::{ErrorResponse, Indication, Request, Response};
use rustun::transport::StunTransport;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;
use stun_codec::rfc5766::attributes::ChannelNumber;
use stun_codec::{rfc5389, rfc5766};

const PERMISSION_LIFETIME_SECONDS: u64 = 300;
const CHANNEL_LIFETIME_SECONDS: u64 = PERMISSION_LIFETIME_SECONDS; // FIXME: Use `600` (and refresh permissions)

#[derive(Debug)]
pub struct ClientCore<S, C>
where
    S: StunTransport<Attribute, PeerAddr = ()>,
    C: Transport<PeerAddr = (), SendItem = ChannelData, RecvItem = ChannelData>,
{
    stun_channel: StunChannel<Attribute, S>,
    channel_data_transporter: C,
    auth_params: AuthParams,
    lifetime: Duration,
    permissions: HashMap<IpAddr, Option<AsyncReply<()>>>,
    channels: HashMap<SocketAddr, ChannelState>,
    next_channel_number: ChannelNumber,
    timeout_queue: TimeoutQueue<TimeoutEntry>,
    refresh_transaction: StunTransaction,
    create_permission_transaction: StunTransaction<(SocketAddr, Response<Attribute>)>,
    channel_bind_transaction: StunTransaction<(SocketAddr, Response<Attribute>)>,
    relay_addr: Option<SocketAddr>,
}
impl<S, C> ClientCore<S, C>
where
    S: StunTransport<Attribute, PeerAddr = ()> + 'static,
    C: Transport<PeerAddr = (), SendItem = ChannelData, RecvItem = ChannelData>,
{
    pub fn allocate(
        stun_transporter: S,
        channel_data_transporter: C,
        auth_params: AuthParams,
    ) -> Allocate<S, C> {
        Allocate::new(
            StunChannel::new(stun_transporter),
            channel_data_transporter,
            auth_params,
        )
    }

    pub fn new(
        stun_channel: StunChannel<Attribute, S>,
        channel_data_transporter: C,
        auth_params: AuthParams,
        lifetime: Duration,
        relay_addr: Option<SocketAddr>,
    ) -> Self {
        let mut timeout_queue = TimeoutQueue::new();
        timeout_queue.push(TimeoutEntry::Refresh, lifetime * 9 / 10);
        ClientCore {
            stun_channel,
            channel_data_transporter,
            auth_params,
            lifetime,
            permissions: HashMap::new(),
            channels: HashMap::new(),
            next_channel_number: ChannelNumber::min(),
            timeout_queue,
            refresh_transaction: StunTransaction::empty(),
            create_permission_transaction: StunTransaction::empty(),
            channel_bind_transaction: StunTransaction::empty(),
            relay_addr,
        }
    }

    pub fn stun_channel_ref(&self) -> &StunChannel<Attribute, S> {
        &self.stun_channel
    }

    pub fn relay_addr(&self) -> Option<SocketAddr> {
        self.relay_addr
    }

    fn start_refresh(&mut self) -> Result<()> {
        let lifetime = track!(rfc5766::attributes::Lifetime::new(self.lifetime))?;

        let mut request = Request::new(rfc5766::methods::REFRESH);
        request.add_attribute(lifetime.into());
        track!(self.auth_params.add_auth_attributes(&mut request))?;

        self.refresh_transaction = StunTransaction::new(self.stun_channel.call((), request));
        Ok(())
    }

    fn handle_refresh_response(&mut self, response: Response<Attribute>) -> Result<()> {
        match response {
            Err(response) => {
                track!(self.handle_error_response(response))?;
                track!(self.start_refresh())?;
            }
            Ok(response) => {
                let mut lifetime = None;
                for attr in response.attributes() {
                    match attr {
                        Attribute::Lifetime(a) => {
                            lifetime = Some(a.lifetime());
                        }
                        Attribute::MessageIntegrity(a) => {
                            track!(self.auth_params.validate(a))?;
                        }
                        _ => {}
                    }
                }

                self.lifetime = track_assert_some!(lifetime, ErrorKind::Other; response);
                self.timeout_queue
                    .push(TimeoutEntry::Refresh, self.lifetime * 9 / 10);
            }
        }
        Ok(())
    }

    fn handle_create_permission_response(
        &mut self,
        peer: SocketAddr,
        response: Response<Attribute>,
    ) -> Result<()> {
        let reply = track_assert_some!(self.permissions.remove(&peer.ip()), ErrorKind::Other);
        match response {
            Err(response) => {
                track!(self.handle_error_response(response))?;
                if let Err(e) = track!(self.create_permission_inner(peer)) {
                    if let Some(reply) = reply {
                        reply.send(Err(e.clone()));
                    }
                    return Err(e);
                }
                self.permissions.insert(peer.ip(), reply);
            }
            Ok(response) => {
                for attr in response.attributes() {
                    if let Attribute::MessageIntegrity(a) = attr {
                        track!(self.auth_params.validate(a))?;
                    }
                }
                if let Some(reply) = reply {
                    reply.send(Ok(()));
                }
                self.permissions.insert(peer.ip(), None);
                self.timeout_queue.push(
                    TimeoutEntry::Permission { peer },
                    Duration::from_secs(PERMISSION_LIFETIME_SECONDS * 9 / 10),
                );
            }
        }
        Ok(())
    }

    fn handle_channel_bind_response(
        &mut self,
        peer: SocketAddr,
        response: Response<Attribute>,
    ) -> Result<()> {
        let state = track_assert_some!(self.channels.remove(&peer), ErrorKind::Other);
        match response {
            Err(response) => {
                track!(self.handle_error_response(response))?;
                if let Err(e) = track!(self.channel_bind_inner(peer, state.channel_number())) {
                    if let ChannelState::Creating { reply, .. } = state {
                        reply.send(Err(e.clone()));
                    }
                    return Err(e);
                }
                self.channels.insert(peer, state);
            }
            Ok(response) => {
                for attr in response.attributes() {
                    if let Attribute::MessageIntegrity(a) = attr {
                        track!(self.auth_params.validate(a))?;
                    }
                }

                let number = state.channel_number();
                if let ChannelState::Creating { reply, .. } = state {
                    reply.send(Ok(()));
                }
                self.channels.insert(peer, ChannelState::Created { number });
                self.timeout_queue.push(
                    TimeoutEntry::Channel { peer },
                    Duration::from_secs(CHANNEL_LIFETIME_SECONDS * 9 / 10),
                );
            }
        }
        Ok(())
    }

    fn handle_error_response(&mut self, response: ErrorResponse<Attribute>) -> Result<()> {
        let error: &rfc5389::attributes::ErrorCode =
            track_assert_some!(response.get_attribute(), ErrorKind::Other; response);
        track_assert_eq!(
            error.code(),
            rfc5389::errors::StaleNonce::CODEPOINT,
            ErrorKind::Other; response
        );

        let nonce: &rfc5389::attributes::Nonce =
            track_assert_some!(response.get_attribute(), ErrorKind::Other; response);
        self.auth_params.set_nonce(nonce.clone());

        Ok(())
    }

    fn handle_timeout(&mut self, entry: TimeoutEntry) -> Result<()> {
        match entry {
            TimeoutEntry::Refresh => track!(self.start_refresh())?,
            TimeoutEntry::Permission { peer } => {
                if self.permissions.remove(&peer.ip()).is_some() {
                    track!(self.create_permission_inner(peer))?;
                    self.permissions.insert(peer.ip(), None);
                    self.timeout_queue.push(
                        TimeoutEntry::Permission { peer },
                        Duration::from_secs(PERMISSION_LIFETIME_SECONDS * 9 / 10),
                    );
                }
            }
            TimeoutEntry::Channel { peer } => {
                if let Some(state) = self.channels.remove(&peer) {
                    track!(self.channel_bind_inner(peer, state.channel_number()))?;
                    self.channels.insert(peer, state);
                    self.timeout_queue.push(
                        TimeoutEntry::Channel { peer },
                        Duration::from_secs(CHANNEL_LIFETIME_SECONDS * 9 / 10),
                    );
                }
            }
        }
        Ok(())
    }

    fn handle_stun_message(
        &mut self,
        message: RecvMessage<Attribute>,
    ) -> Result<Option<(SocketAddr, Vec<u8>)>> {
        match message {
            RecvMessage::Invalid(message) => track_panic!(ErrorKind::Other; message),
            RecvMessage::Request(request) => track_panic!(ErrorKind::Other; request),
            RecvMessage::Indication(indication) => track!(self.handle_stun_indication(indication)),
        }
    }

    fn handle_stun_indication(
        &mut self,
        indication: Indication<Attribute>,
    ) -> Result<Option<(SocketAddr, Vec<u8>)>> {
        match indication.method() {
            rfc5766::methods::DATA => {
                let data: &rfc5766::attributes::Data =
                    track_assert_some!(indication.get_attribute(), ErrorKind::Other; indication);
                let peer: &rfc5766::attributes::XorPeerAddress =
                    track_assert_some!(indication.get_attribute(), ErrorKind::Other; indication);
                track_assert!(
                    self.permissions.contains_key(&peer.address().ip()),
                    ErrorKind::Other; peer,  indication
                );
                Ok(Some((peer.address(), Vec::from(data.data()))))
            }
            _ => {
                track_panic!(ErrorKind::Other; indication);
            }
        }
    }

    fn handle_channel_data(&mut self, data: ChannelData) -> Result<(SocketAddr, Vec<u8>)> {
        // FIXME: optimize
        let peer = track_assert_some!(
            self.channels
                .iter()
                .find(|x| x.1.channel_number() == data.channel_number())
                .map(|x| *x.0),
            ErrorKind::Other
        );
        Ok((peer, data.into_data()))
    }

    fn create_permission_inner(&mut self, peer: SocketAddr) -> Result<()> {
        // If a permission already exists on the TURN server, it will just be refreshed.
        let mut request = Request::new(rfc5766::methods::CREATE_PERMISSION);
        request.add_attribute(rfc5766::attributes::XorPeerAddress::new(peer).into());
        track!(self.auth_params.add_auth_attributes(&mut request))?;

        self.create_permission_transaction =
            StunTransaction::with_peer(peer, self.stun_channel.call((), request));
        Ok(())
    }

    fn channel_bind_inner(
        &mut self,
        peer: SocketAddr,
        channel_number: ChannelNumber,
    ) -> Result<()> {
        track_assert!(!self.channels.contains_key(&peer), ErrorKind::InvalidInput; peer);

        let mut request = Request::new(rfc5766::methods::CHANNEL_BIND);
        request.add_attribute(rfc5766::attributes::XorPeerAddress::new(peer).into());
        request.add_attribute(channel_number.into());
        track!(self.auth_params.add_auth_attributes(&mut request))?;

        self.channel_bind_transaction =
            StunTransaction::with_peer(peer, self.stun_channel.call((), request));
        Ok(())
    }

    fn next_channel_number(&mut self) -> ChannelNumber {
        // FIXME: collision check
        let curr = self.next_channel_number;
        self.next_channel_number = curr.wrapping_increment();
        curr
    }

    pub fn create_permission(&mut self, peer: SocketAddr) -> AsyncResult<()> {
        let (result, reply) = AsyncResult::new();
        match track!(self.create_permission_inner(peer)) {
            Err(e) => {
                reply.send(Err(e));
            }
            Ok(()) => {
                self.permissions.insert(peer.ip(), Some(reply));
            }
        }
        result
    }

    pub fn channel_bind(&mut self, peer: SocketAddr) -> AsyncResult<()> {
        let (result, reply) = AsyncResult::new();
        let channel_number = self.next_channel_number();
        match track!(self.channel_bind_inner(peer, channel_number)) {
            Err(e) => {
                reply.send(Err(e));
            }
            Ok(()) => {
                self.channels.insert(
                    peer,
                    ChannelState::Creating {
                        number: channel_number,
                        reply,
                    },
                );
            }
        }
        result
    }

    pub fn start_send(&mut self, peer: SocketAddr, data: Vec<u8>) -> Result<()> {
        if let Some(state) = self.channels.get(&peer) {
            let data = track!(ChannelData::new(state.channel_number(), data,))?;
            track!(self.channel_data_transporter.start_send((), data))?;
        } else if self.permissions.contains_key(&peer.ip()) {
            track_assert!(self.permissions.contains_key(&peer.ip()), ErrorKind::Other; peer);
            let mut indication = Indication::new(rfc5766::methods::SEND);
            indication.add_attribute(rfc5766::attributes::XorPeerAddress::new(peer).into());
            indication.add_attribute(track!(rfc5766::attributes::Data::new(data))?.into());
            track!(self.stun_channel.cast((), indication))?;
        } else {
            track_panic!(ErrorKind::InvalidInput, "Unknown peer: {:?}", peer);
        }
        Ok(())
    }

    pub fn poll_send(&mut self) -> Poll<(), Error> {
        let is_ready = track!(self.stun_channel.poll_send())?.is_ready()
            && track!(self.channel_data_transporter.poll_send())?.is_ready();
        if is_ready {
            Ok(Async::Ready(()))
        } else {
            Ok(Async::NotReady)
        }
    }

    pub fn poll_recv(&mut self) -> Poll<Option<(SocketAddr, Vec<u8>)>, Error> {
        let mut did_something = true;
        while did_something {
            did_something = false;

            while let Async::Ready(message) = track!(self.stun_channel.poll_recv())? {
                did_something = true;
                if let Some((_, message)) = message {
                    if let Some((peer, data)) = track!(self.handle_stun_message(message))? {
                        return Ok(Async::Ready(Some((peer, data))));
                    }
                } else {
                    track_panic!(ErrorKind::Other, "Unexpected termination");
                }
            }
            if let Async::Ready(data) = track!(self.channel_data_transporter.poll_recv())? {
                if let Some((_, data)) = data {
                    let (peer, data) = track!(self.handle_channel_data(data))?;
                    return Ok(Async::Ready(Some((peer, data))));
                } else {
                    track_panic!(ErrorKind::Other, "Unexpected termination");
                }
            }
            while let Some(entry) = self.timeout_queue.pop() {
                did_something = true;
                track!(self.handle_timeout(entry))?;
            }
            if let Async::Ready(response) = track!(self.refresh_transaction.poll())? {
                did_something = true;
                track!(self.handle_refresh_response(response))?;
            }
            if let Async::Ready((peer, response)) =
                track!(self.create_permission_transaction.poll())?
            {
                did_something = true;
                track!(self.handle_create_permission_response(peer, response))?;
            }
            if let Async::Ready((peer, response)) = track!(self.channel_bind_transaction.poll())? {
                did_something = true;
                track!(self.handle_channel_bind_response(peer, response))?;
            }
            track!(self.channel_data_transporter.poll_send())?;
        }
        Ok(Async::NotReady)
    }
}

#[derive(Debug)]
enum TimeoutEntry {
    Refresh,
    Permission { peer: SocketAddr },
    Channel { peer: SocketAddr },
}

#[derive(Debug)]
enum ChannelState {
    Creating {
        number: ChannelNumber,
        reply: AsyncReply<()>,
    },
    Created {
        number: ChannelNumber,
    },
}
impl ChannelState {
    fn channel_number(&self) -> ChannelNumber {
        match self {
            ChannelState::Creating { number, .. } => *number,
            ChannelState::Created { number } => *number,
        }
    }
}
