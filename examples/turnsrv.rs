#[macro_use]
extern crate trackable;

use clap::Parser;
use rusturn::auth::AuthParams;
use rusturn::server::UdpServer;
use std::net::SocketAddr;

#[derive(Debug, Parser)]
#[clap(name = "turnsrv")]
struct Opt {
    /// STUN server address.
    #[clap(long, default_value = "127.0.0.1:3478")]
    server: SocketAddr,

    /// Username.
    #[clap(long, default_value = "foo")]
    username: String,

    /// Password.
    #[clap(long, default_value = "bar")]
    password: String,

    /// Realm.
    #[clap(long, default_value = "baz")]
    realm: String,

    /// Nonce.
    #[clap(long, default_value = "qux")]
    nonce: String,
}

fn main() -> Result<(), trackable::error::MainError> {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    let opt = Opt::parse();

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
