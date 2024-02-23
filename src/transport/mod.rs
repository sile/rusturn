use crate::attribute::Attribute;
use crate::turn_message::{TurnMessageDecoder, TurnMessageEncoder};
use fibers_transport::{TcpTransporter, UdpTransporter};

pub(crate) use self::channel_data::ChannelDataTransporter;
pub(crate) use self::stun::StunTransporter;

pub use self::udp_over_turn::UdpOverTurnTransporter;

mod channel_data;
mod stun;
mod udp_over_turn;

pub(crate) type StunTcpTransporter =
    rustun::transport::StunTcpTransporter<StunTransporter<TurnTcpTransporter>>;

pub(crate) type StunUdpTransporter =
    rustun::transport::StunUdpTransporter<Attribute, StunTransporter<TurnUdpTransporter>>;

pub(crate) type ChannelDataTcpTransporter = ChannelDataTransporter<TurnTcpTransporter>;

pub(crate) type ChannelDataUdpTransporter = ChannelDataTransporter<TurnUdpTransporter>;

pub(crate) type TurnUdpTransporter = UdpTransporter<TurnMessageEncoder, TurnMessageDecoder>;

pub(crate) type TurnTcpTransporter = TcpTransporter<TurnMessageEncoder, TurnMessageDecoder>;
