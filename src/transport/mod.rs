use fibers_transport::{TcpTransporter, UdpTransporter};
use rustun;

use attribute::Attribute;
use turn_message::{TurnMessageDecoder, TurnMessageEncoder};
//use client::Client;

pub use self::channel_data::ChannelDataTransporter;
pub use self::stun::StunTransporter;

mod channel_data;
mod stun;

pub type StunTcpTransporter =
    rustun::transport::StunTcpTransporter<StunTransporter<TurnTcpTransporter>>;

pub type StunUdpTransporter =
    rustun::transport::StunUdpTransporter<Attribute, StunTransporter<TurnUdpTransporter>>;

pub type ChannelDataTcpTransporter = ChannelDataTransporter<TurnTcpTransporter>;

pub type ChannelDataUdpTransporter = ChannelDataTransporter<TurnUdpTransporter>;

pub type TurnUdpTransporter = UdpTransporter<TurnMessageEncoder, TurnMessageDecoder>;

pub type TurnTcpTransporter = TcpTransporter<TurnMessageEncoder, TurnMessageDecoder>;

// TODO
// #[derive(Debug)]
// pub struct UdpOverTurnTransporter<C, E, D> {
//     client: C,
//     encoder: E,
//     decoder: D,
//     channels: HashSet<SocketAddr>,
//     last_error: Option<Error>,
// }
// impl<C, E, D> UdpOverTurnTransporter<C, E, D>
// where
//     C: Client,
//     E: Encode + Default,
//     D: Decode + Default,
// {
//     pub fn new(client: C) -> Self {
//         UdpOverTurnTransporter {
//             client,
//             encoder: E::default(),
//             decoder: D::default(),
//             channels: HashSet::new(),
//             last_error: None,
//         }
//     }

//     fn ensure_channel_exists(&mut self, peer: SocketAddr) {
//         if self.channels.insert(peer) {
//             self.client.channel_bind(peer);
//         }
//     }
// }
// impl<C, E, D> Transport for UdpOverTurnTransporter<C, E, D>
// where
//     C: Client,
//     E: Encode + Default,
//     D: Decode + Default,
// {
//     type SendItem = E::Item;
//     type RecvItem = D::Item;

//     fn send(&mut self, peer: SocketAddr, message: Self::SendItem) {
//         if self.last_error.is_some() {
//             return;
//         }

//         self.ensure_channel_exists(peer);
//         if let Err(e) = track!(self.encoder.encode_into_bytes(message).map_err(Error::from))
//             .and_then(|data| track!(self.client.send_channel_data(peer, data)))
//         {
//             self.last_error = Some(Error::from(e));
//         }
//     }

//     fn recv(&mut self) -> Option<(SocketAddr, Self::RecvItem)> {
//         if self.last_error.is_some() {
//             None
//         } else if let Some((peer, data)) = self.client.recv_data() {
//             match track!(self.decoder.decode_from_bytes(&data)) {
//                 Err(e) => {
//                     self.last_error = Some(Error::from(e));
//                     None
//                 }
//                 Ok(item) => Some((peer, item)),
//             }
//         } else {
//             None
//         }
//     }

//     fn run_once(&mut self) -> Result<bool> {
//         if let Some(e) = self.last_error.take() {
//             return Err(e);
//         }
//         track!(self.client.run_once())?;
//         Ok(false)
//     }
// }
// impl<C, E, D> UnreliableTransport for UdpOverTurnTransporter<C, E, D>
// where
//     C: Client,
//     E: Encode + Default,
//     D: Decode + Default,
// {}
