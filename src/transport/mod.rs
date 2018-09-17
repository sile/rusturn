use fibers_transport::{TcpTransporter, UdpTransporter};
use rustun;

use attribute::Attribute;
use turn_message::{TurnMessageDecoder, TurnMessageEncoder};

pub use self::channel_data::ChannelDataTransporter;
pub use self::stun::StunTransporter;
pub use self::udp_over_turn::UdpOverTurnTransporter;

mod channel_data;
mod stun;
mod udp_over_turn;

pub type StunTcpTransporter =
    rustun::transport::StunTcpTransporter<StunTransporter<TurnTcpTransporter>>;

pub type StunUdpTransporter =
    rustun::transport::StunUdpTransporter<Attribute, StunTransporter<TurnUdpTransporter>>;

pub type ChannelDataTcpTransporter = ChannelDataTransporter<TurnTcpTransporter>;

pub type ChannelDataUdpTransporter = ChannelDataTransporter<TurnUdpTransporter>;

pub type TurnUdpTransporter = UdpTransporter<TurnMessageEncoder, TurnMessageDecoder>;

pub type TurnTcpTransporter = TcpTransporter<TurnMessageEncoder, TurnMessageDecoder>;
