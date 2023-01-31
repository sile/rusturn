use crate::attribute::Attribute;
use crate::auth::AuthParams;
use crate::channel_data::ChannelData;
use crate::{Error, ErrorKind, Result};
use fibers_timeout_queue::TimeoutQueue;
use fibers_transport::Transport;
use futures::{Async, Future, Poll};
use rustun::channel::{Channel as StunChannel, RecvMessage};
use rustun::message::{ErrorResponse, Indication, Request, SuccessResponse};
use rustun::transport::StunTransport;
use std::collections::HashMap;
use std::net::UdpSocket as StdUdpSocket;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;
use stun_codec::rfc5766::attributes::ChannelNumber;
use stun_codec::{rfc5389, rfc5766};

const ALLOCATION_LIEFTIME_SECONDS: u64 = 600;
const PERMISSION_LIFETIME_SECONDS: u64 = 300;
const CHANNEL_LIEFTIME_SECONDS: u64 = 600;

#[derive(Debug)]
pub struct ServerCore<S, C>
where
    S: StunTransport<Attribute, PeerAddr = SocketAddr>,
    C: Transport<PeerAddr = SocketAddr, SendItem = ChannelData, RecvItem = ChannelData>,
{
    stun_channel: StunChannel<Attribute, S>,
    channel_data_transporter: C,
    allocations: HashMap<SocketAddr, AllocationState>,
    seqno: u64,
    auth_params: AuthParams,
    timeout_queue: TimeoutQueue<TimeoutEntry>,
}
impl<S, C> ServerCore<S, C>
where
    S: StunTransport<Attribute, PeerAddr = SocketAddr>,
    C: Transport<PeerAddr = SocketAddr, SendItem = ChannelData, RecvItem = ChannelData>,
{
    pub fn new(stun_transporter: S, channel_data_transporter: C, auth_params: AuthParams) -> Self {
        let mut timeout_queue = TimeoutQueue::new();
        timeout_queue.push(TimeoutEntry::PollRecv, Duration::from_millis(100));
        ServerCore {
            stun_channel: StunChannel::new(stun_transporter),
            channel_data_transporter,
            allocations: HashMap::new(),
            seqno: 0,
            auth_params,
            timeout_queue,
        }
    }

    pub fn stun_transporter_ref(&self) -> &S {
        self.stun_channel.transporter_ref()
    }

    fn handle_stun_message(
        &mut self,
        client: SocketAddr,
        message: RecvMessage<Attribute>,
    ) -> Result<()> {
        match message {
            RecvMessage::Request(m) => {
                track!(self.handle_stun_request(client, m))?;
            }
            RecvMessage::Indication(m) => {
                track!(self.handle_stun_indication(client, m))?;
            }
            RecvMessage::Invalid(m) => {
                #[allow(clippy::print_literal)]
                {
                    eprintln!("[ERROR:{}:{}] Invalid message: {:?}", file!(), line!(), m);
                }
            }
        }
        Ok(())
    }

    fn handle_stun_request(
        &mut self,
        client: SocketAddr,
        request: Request<Attribute>,
    ) -> Result<()> {
        match request.method() {
            rfc5766::methods::ALLOCATE => track!(self.handle_allocate(client, request))?,
            rfc5766::methods::REFRESH => track!(self.handle_refresh(client, request))?,
            rfc5766::methods::CREATE_PERMISSION => {
                track!(self.handle_create_permission(client, request))?
            }
            rfc5766::methods::CHANNEL_BIND => track!(self.handle_channel_bind(client, request))?,
            _ => track!(self.reply_bad_request(client, request))?,
        }
        Ok(())
    }

    fn auth_validate(&self, request: &Request<Attribute>) -> Result<()> {
        let mi = track_assert_some!(
            request.get_attribute::<rfc5389::attributes::MessageIntegrity>(),
            ErrorKind::InvalidInput
        );
        track!(self.auth_params.validate(mi))?;
        Ok(())
    }

    fn handle_create_permission(
        &mut self,
        client: SocketAddr,
        request: Request<Attribute>,
    ) -> Result<()> {
        track!(self.auth_validate(&request))?;
        let peer = track_assert_some!(
            request.get_attribute::<rfc5766::attributes::XorPeerAddress>(),
            ErrorKind::InvalidInput
        )
        .address();

        let seqno = self.next_seqno();
        let allocation =
            track_assert_some!(self.allocations.get_mut(&client), ErrorKind::InvalidInput);

        allocation
            .permissions
            .entry(peer.ip())
            .or_insert_with(|| PermissionState { seqno })
            .seqno = seqno;

        let mut response = SuccessResponse::new(&request);
        track!(self.auth_params.add_auth_attributes(&mut response))?;
        track!(self.stun_channel.reply(client, Ok(response)))?;

        self.timeout_queue.push(
            TimeoutEntry::Permission {
                client,
                peer: peer.ip(),
                seqno,
            },
            Duration::from_secs(PERMISSION_LIFETIME_SECONDS),
        );

        Ok(())
    }

    fn handle_channel_bind(
        &mut self,
        client: SocketAddr,
        request: Request<Attribute>,
    ) -> Result<()> {
        track!(self.auth_validate(&request))?;
        let peer = track_assert_some!(
            request.get_attribute::<rfc5766::attributes::XorPeerAddress>(),
            ErrorKind::InvalidInput
        )
        .address();
        let channel_number = *track_assert_some!(
            request.get_attribute::<ChannelNumber>(),
            ErrorKind::InvalidInput
        );

        let seqno = self.next_seqno();
        let allocation =
            track_assert_some!(self.allocations.get_mut(&client), ErrorKind::InvalidInput);

        allocation
            .channels
            .entry(channel_number)
            .or_insert_with(|| ChannelState::new(peer, seqno))
            .seqno = seqno;

        let mut response = SuccessResponse::new(&request);
        track!(self.auth_params.add_auth_attributes(&mut response))?;
        track!(self.stun_channel.reply(client, Ok(response)))?;

        self.timeout_queue.push(
            TimeoutEntry::Channel {
                client,
                channel_number,
                seqno,
            },
            Duration::from_secs(CHANNEL_LIEFTIME_SECONDS),
        );

        Ok(())
    }

    fn handle_refresh(&mut self, client: SocketAddr, request: Request<Attribute>) -> Result<()> {
        track!(self.auth_validate(&request))?;
        let lifetime = track_assert_some!(
            request.get_attribute::<rfc5766::attributes::Lifetime>(),
            ErrorKind::InvalidInput
        );
        if lifetime.lifetime().as_secs() == 0 {
            self.allocations.remove(&client);
        } else {
            let seqno = self.next_seqno();
            let allocation =
                track_assert_some!(self.allocations.get_mut(&client), ErrorKind::InvalidInput);
            allocation.seqno = seqno;
            self.timeout_queue.push(
                TimeoutEntry::Allocation { client, seqno },
                lifetime.lifetime(),
            );

            let mut response = SuccessResponse::new(&request);
            response.add_attribute(lifetime.clone().into());
            track!(self.auth_params.add_auth_attributes(&mut response))?;
            track!(self.stun_channel.reply(client, Ok(response)))?;
        }
        Ok(())
    }

    fn handle_allocate(&mut self, client: SocketAddr, request: Request<Attribute>) -> Result<()> {
        if let Some(mi) = request.get_attribute::<rfc5389::attributes::MessageIntegrity>() {
            track!(self.auth_params.validate(mi))?;

            // FIXME: Add existence check
            let seqno = self.next_seqno();
            let state = track!(AllocationState::new(seqno))?;
            let relay_addr = track!(state.socket.local_addr().map_err(Error::from))?;
            self.allocations.insert(client, state);

            let lifetime = Duration::from_secs(ALLOCATION_LIEFTIME_SECONDS);
            self.timeout_queue
                .push(TimeoutEntry::Allocation { client, seqno }, lifetime);

            let mut response = SuccessResponse::new(&request);
            response.add_attribute(track!(rfc5766::attributes::Lifetime::new(lifetime))?.into());
            response.add_attribute(rfc5766::attributes::XorRelayAddress::new(relay_addr).into());
            track!(self.auth_params.add_auth_attributes(&mut response))?;
            track!(self.stun_channel.reply(client, Ok(response)))?;
        } else {
            let realm = track_assert_some!(self.auth_params.get_realm(), ErrorKind::InvalidInput);
            let nonce = track_assert_some!(self.auth_params.get_nonce(), ErrorKind::InvalidInput);

            let mut response = ErrorResponse::new(&request, rfc5389::errors::Unauthorized.into());
            response.add_attribute(realm.clone().into());
            response.add_attribute(nonce.clone().into());
            track!(self.stun_channel.reply(client, Err(response)))?;
        }
        Ok(())
    }

    fn handle_stun_indication(
        &mut self,
        client: SocketAddr,
        indication: Indication<Attribute>,
    ) -> Result<()> {
        match indication.method() {
            rfc5766::methods::SEND => {
                track!(self.handle_send(client, indication))?;
            }
            _ => {
                #[allow(clippy::print_literal)]
                {
                    eprintln!(
                        "[WARN:{}:{}] Unknown STUN indication: {:?}",
                        file!(),
                        line!(),
                        indication
                    );
                }
            }
        }
        Ok(())
    }

    fn handle_send(&mut self, client: SocketAddr, indication: Indication<Attribute>) -> Result<()> {
        let allocation =
            track_assert_some!(self.allocations.get_mut(&client), ErrorKind::InvalidInput);
        let peer = track_assert_some!(
            indication.get_attribute::<rfc5766::attributes::XorPeerAddress>(),
            ErrorKind::InvalidInput
        )
        .address();
        let data = track_assert_some!(
            indication.get_attribute::<rfc5766::attributes::Data>(),
            ErrorKind::InvalidInput
        );
        track_assert!(
            allocation.permissions.contains_key(&peer.ip()),
            ErrorKind::InvalidInput
        );
        track!(allocation
            .socket
            .send_to(data.data(), peer)
            .map_err(Error::from))?;

        Ok(())
    }

    fn reply_bad_request(&mut self, client: SocketAddr, request: Request<Attribute>) -> Result<()> {
        let response = ErrorResponse::new(&request, rfc5389::errors::BadRequest.into());
        track!(self.stun_channel.reply(client, Err(response)))?;
        Ok(())
    }

    fn handle_channel_data(&mut self, client: SocketAddr, data: ChannelData) -> Result<()> {
        let allocation =
            track_assert_some!(self.allocations.get_mut(&client), ErrorKind::InvalidInput);
        let channel = track_assert_some!(
            allocation.channels.get_mut(&data.channel_number()),
            ErrorKind::InvalidInput
        );
        track!(allocation
            .socket
            .send_to(&data.into_data(), channel.peer_addr)
            .map_err(Error::from))?;
        Ok(())
    }

    fn next_seqno(&mut self) -> u64 {
        let n = self.seqno;
        self.seqno += 1;
        n
    }

    fn handle_timeout(&mut self, entry: TimeoutEntry) -> Result<()> {
        match entry {
            TimeoutEntry::Allocation { client, seqno } => {
                let do_delete = self
                    .allocations
                    .get(&client)
                    .map_or(false, |s| s.seqno == seqno);
                if do_delete {
                    self.allocations.remove(&client);
                }
            }
            TimeoutEntry::Permission {
                client,
                peer,
                seqno,
            } => {
                if let Some(allocation) = self.allocations.get_mut(&client) {
                    let do_delete = allocation
                        .permissions
                        .get_mut(&peer)
                        .map_or(false, |s| s.seqno == seqno);
                    if do_delete {
                        allocation.permissions.remove(&peer);
                    }
                }
            }
            TimeoutEntry::Channel {
                client,
                channel_number,
                seqno,
            } => {
                // FIXME: Check permission lifetime
                if let Some(allocation) = self.allocations.get_mut(&client) {
                    let do_delete = allocation
                        .channels
                        .get_mut(&channel_number)
                        .map_or(false, |s| s.seqno == seqno);
                    if do_delete {
                        allocation.channels.remove(&channel_number);
                    }
                }
            }
            TimeoutEntry::PollRecv => {
                // FIXME: Use asynchronous UDP socket
                track!(self.poll_peer_recv())?;
                self.timeout_queue
                    .push(TimeoutEntry::PollRecv, Duration::from_millis(100));
            }
        }
        Ok(())
    }

    fn poll_peer_recv(&mut self) -> Result<()> {
        let mut buf = [0; 4096];
        for (client, allocation) in &mut self.allocations {
            match allocation.socket.recv_from(&mut buf) {
                Err(e) => {
                    use std::io;
                    track_assert!(
                        e.kind() == io::ErrorKind::WouldBlock,
                        ErrorKind::Other,
                        "I/O Error: {}",
                        e
                    );
                }
                Ok((size, peer)) => {
                    let data = Vec::from(&buf[..size]);

                    // FIXME: optimize
                    if let Some(&channel_number) = allocation
                        .channels
                        .iter()
                        .find(|(_, s)| s.peer_addr == peer)
                        .map(|x| x.0)
                    {
                        let data = track!(ChannelData::new(channel_number, data))?;
                        track!(self.channel_data_transporter.start_send(*client, data))?;
                    } else {
                        track_assert!(
                            allocation.permissions.contains_key(&peer.ip()),
                            ErrorKind::InvalidInput
                        );

                        let mut indication = Indication::new(rfc5766::methods::DATA);
                        indication
                            .add_attribute(rfc5766::attributes::XorPeerAddress::new(peer).into());
                        indication
                            .add_attribute(track!(rfc5766::attributes::Data::new(data))?.into());
                        track!(self.stun_channel.cast(*client, indication))?;
                    }
                }
            }
        }
        Ok(())
    }
}
unsafe impl<S, C> Send for ServerCore<S, C>
where
    S: StunTransport<Attribute, PeerAddr = SocketAddr>,
    C: Transport<PeerAddr = SocketAddr, SendItem = ChannelData, RecvItem = ChannelData>,
{
}
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
                if let Some((client, message)) = message {
                    track!(self.handle_stun_message(client, message))?;
                } else {
                    return Ok(Async::Ready(()));
                }
            }
            while let Async::Ready(data) = track!(self.channel_data_transporter.poll_recv())? {
                did_something = true;
                if let Some((client, data)) = data {
                    track!(self.handle_channel_data(client, data))?;
                } else {
                    return Ok(Async::Ready(()));
                }
            }
            track!(self.stun_channel.poll_send())?;
            track!(self.channel_data_transporter.poll_send())?;
            while let Some(entry) = self.timeout_queue.pop() {
                did_something = true;
                track!(self.handle_timeout(entry))?;
            }
        }
        Ok(Async::NotReady)
    }
}

#[derive(Debug)]
struct AllocationState {
    seqno: u64,
    socket: StdUdpSocket,
    permissions: HashMap<IpAddr, PermissionState>,
    channels: HashMap<ChannelNumber, ChannelState>,
}
impl AllocationState {
    fn new(seqno: u64) -> Result<Self> {
        // FIXME: Make asynchronous
        let socket = track!(StdUdpSocket::bind("0.0.0.0:0").map_err(Error::from))?;
        track!(socket.set_nonblocking(true).map_err(Error::from))?;
        Ok(AllocationState {
            seqno,
            socket,
            permissions: HashMap::new(),
            channels: HashMap::new(),
        })
    }
}

#[derive(Debug)]
struct ChannelState {
    peer_addr: SocketAddr,
    seqno: u64,
}
impl ChannelState {
    fn new(peer_addr: SocketAddr, seqno: u64) -> Self {
        ChannelState { peer_addr, seqno }
    }
}

#[derive(Debug)]
struct PermissionState {
    seqno: u64,
}

#[derive(Debug)]
enum TimeoutEntry {
    Allocation {
        client: SocketAddr,
        seqno: u64,
    },
    Permission {
        client: SocketAddr,
        peer: IpAddr,
        seqno: u64,
    },
    Channel {
        client: SocketAddr,
        channel_number: ChannelNumber,
        seqno: u64,
    },
    PollRecv,
}
