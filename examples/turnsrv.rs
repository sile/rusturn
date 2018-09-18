extern crate fibers_global;
extern crate rusturn;
#[macro_use]
extern crate structopt;
#[macro_use]
extern crate trackable;

use rusturn::auth::AuthParams;
use rusturn::server::UdpServer;
use std::net::SocketAddr;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "turncli")]
struct Opt {
    /// STUN server address.
    #[structopt(long = "server", default_value = "127.0.0.1:3478")]
    server: SocketAddr,

    /// Username.
    #[structopt(long = "username", default_value = "foo")]
    username: String,

    /// Password.
    #[structopt(long = "password", default_value = "bar")]
    password: String,

    /// Realm.
    #[structopt(long = "realm", default_value = "baz")]
    realm: String,

    /// Nonce.
    #[structopt(long = "nonce", default_value = "qux")]
    nonce: String,
}

fn main() -> Result<(), trackable::error::MainError> {
    let opt = Opt::from_args();

    let server_addr = opt.server;
    let auth_params = track!(AuthParams::with_realm_and_nonce(
        &opt.username,
        &opt.password,
        &opt.realm,
        &opt.nonce
    ))?;

    let turn_server = track!(fibers_global::execute(UdpServer::start(
        server_addr,
        auth_params,
    )))?;
    track!(fibers_global::execute(turn_server))?;

    Ok(())
}
