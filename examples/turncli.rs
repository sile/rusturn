extern crate fibers;
extern crate fibers_global;
extern crate futures;
extern crate rusturn;
#[macro_use]
extern crate structopt;
#[macro_use]
extern crate trackable;

use futures::{Async, Future, Poll};
use rusturn::auth::AuthParams;
use rusturn::client::{Client, UdpClient};
use rusturn::Error;
use std::io::Read;
use std::net::SocketAddr;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "turncli")]
struct Opt {
    /// STUN server address.
    #[structopt(long = "server", default_value = "127.0.0.1:3478")]
    server: SocketAddr,

    /// Peer address.
    #[structopt(long = "peer", default_value = "127.0.0.1:2000")]
    peer: SocketAddr,

    /// Username.
    #[structopt(long = "username", default_value = "foo")]
    username: String,

    /// Password.
    #[structopt(long = "password", default_value = "bar")]
    password: String,

    /// Whether to send data using `ChannelData` messages.
    #[structopt(long = "use-channel-data")]
    use_channel_data: bool,
}

fn main() -> Result<(), trackable::error::MainError> {
    let opt = Opt::from_args();

    let server_addr = opt.server;
    let auth_params = track!(AuthParams::new(opt.username.clone(), opt.password.clone()))?;
    let peer = opt.peer;

    let client = track!(fibers_global::execute(UdpClient::allocate(
        server_addr,
        auth_params
    )))?;
    eprintln!("# ALLOCATED: server={:?}", server_addr);

    let use_channel_data = opt.use_channel_data;
    let client = track!(fibers_global::execute(
        client
            .wait(|client| if use_channel_data {
                client.channel_bind(peer)
            } else {
                client.create_permission(peer)
            }).and_then(|(client, result)| result.map(move |_| client))
    ))?;
    if use_channel_data {
        eprintln!("# CHANNEL BOUND: peer={:?}", peer);
    } else {
        eprintln!("# PERMISSION CREATED: peer={:?}", peer);
    }

    track!(fibers_global::execute(SendRecvLoop {
        peer,
        client,
        stdin: fibers::io::stdin(),
        use_channel_data
    }))?;

    Ok(())
}

struct SendRecvLoop {
    peer: SocketAddr,
    client: UdpClient,
    stdin: fibers::io::Stdin,
    use_channel_data: bool,
}
impl Future for SendRecvLoop {
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        while let Some((peer, data)) = self.client.recv_data() {
            println!("# RECV FROM {:?}: {:?}", peer, data);
        }

        let mut buf = [0; 1024];
        match self.stdin.read(&mut buf) {
            Err(e) => {
                if e.kind() != std::io::ErrorKind::WouldBlock {
                    return Err(track!(Error::from(e)));
                }
            }
            Ok(0) => {
                return Ok(Async::Ready(()));
            }
            Ok(size) => {
                let data = Vec::from(&buf[..size]);
                if self.use_channel_data {
                    track!(self.client.send_channel_data(self.peer, data))?;
                } else {
                    track!(self.client.send_data(self.peer, data))?;
                }
            }
        }

        track!(self.client.run_once());
        Ok(Async::NotReady)
    }
}
