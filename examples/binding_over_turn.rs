#[macro_use]
extern crate trackable;

use clap::Parser;
use rustun::channel::Channel;
use rustun::client::Client as StunClient;
use rustun::message::Request;
use rustun::transport::StunUdpTransporter;
use rusturn::auth::AuthParams;
use rusturn::client::UdpClient;
use rusturn::transport::UdpOverTurnTransporter;
use std::net::SocketAddr;
use stun_codec::rfc5389;
use stun_codec::{MessageDecoder, MessageEncoder};

#[derive(Debug, Parser)]
#[clap(name = "turncli")]
struct Opt {
    /// TURN server address.
    #[clap(long)]
    turn_server: SocketAddr,

    /// STUN server address ("peer" in TURN's terminology).
    #[clap(long)]
    stun_server: SocketAddr,

    /// TURN username.
    #[clap(long, default_value = "foo")]
    username: String,

    /// TURN password.
    #[clap(long, default_value = "bar")]
    password: String,
}

fn main() -> Result<(), trackable::error::MainError> {
    let opt = Opt::parse();

    let auth_params = track!(AuthParams::new(&opt.username, &opt.password))?;

    let turn_client = track!(fibers_global::execute(UdpClient::allocate(
        opt.turn_server,
        auth_params
    )))?;
    let transporter =
        UdpOverTurnTransporter::<_, MessageEncoder<_>, MessageDecoder<_>>::new(turn_client);

    let stun_channel = Channel::new(StunUdpTransporter::new(transporter));
    let stun_client = StunClient::new(&fibers_global::handle(), stun_channel);

    let request = Request::<rfc5389::Attribute>::new(rfc5389::methods::BINDING);
    let response = track!(fibers_global::execute(
        stun_client.call(opt.stun_server, request)
    ))?;
    println!("{:?}", response);

    Ok(())
}
