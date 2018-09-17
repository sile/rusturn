extern crate fibers_global;
extern crate futures;
extern crate rustun;
extern crate rusturn;
#[macro_use]
extern crate structopt;
extern crate stun_codec;
#[macro_use]
extern crate trackable;

use rustun::channel::Channel;
use rustun::client::Client as StunClient;
use rustun::message::Request;
use rustun::transport::StunUdpTransporter;
use rusturn::auth::AuthParams;
use rusturn::client::UdpClient;
use rusturn::transport::UdpOverTurnTransporter;
use std::net::SocketAddr;
use structopt::StructOpt;
use stun_codec::rfc5389;
use stun_codec::{MessageDecoder, MessageEncoder};

#[derive(Debug, StructOpt)]
#[structopt(name = "turncli")]
struct Opt {
    /// TURN server address.
    #[structopt(long = "turn-server")]
    turn_server: SocketAddr,

    /// STUN server address ("peer" in TURN's terminology).
    #[structopt(long = "stun-server")]
    stun_server: SocketAddr,

    /// TURN username.
    #[structopt(long = "username", default_value = "foo")]
    username: String,

    /// TURN password.
    #[structopt(long = "password", default_value = "bar")]
    password: String,
}

fn main() -> Result<(), trackable::error::MainError> {
    let opt = Opt::from_args();

    let auth_params = track!(AuthParams::new(opt.username.clone(), opt.password.clone()))?;

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
