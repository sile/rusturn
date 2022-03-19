#[macro_use]
extern crate trackable;

use clap::Parser;
use futures::{Async, Future, Poll};
use rusturn::auth::AuthParams;
use rusturn::client::{wait, Client, UdpClient};
use rusturn::Error;
use std::io::Read;
use std::net::SocketAddr;

#[derive(Debug, Parser)]
#[clap(name = "turncli")]
struct Opt {
    /// STUN server address.
    #[clap(long, default_value = "127.0.0.1:3478")]
    server: SocketAddr,

    /// Peer address.
    #[clap(long, default_value = "127.0.0.1:2000")]
    peer: SocketAddr,

    /// Username.
    #[clap(long, default_value = "foo")]
    username: String,

    /// Password.
    #[clap(long, default_value = "bar")]
    password: String,

    /// Whether to send data using `ChannelData` messages.
    #[clap(long)]
    use_channel_data: bool,

    /// The number of scheduler threads.
    #[clap(long, default_value_t = 1)]
    thread_count: usize,
}

fn main() -> Result<(), trackable::error::MainError> {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    let opt = Opt::parse();
    track_assert!(
        fibers_global::set_thread_count(opt.thread_count),
        trackable::error::Failed
    );

    let server_addr = opt.server;
    let auth_params = track!(AuthParams::new(&opt.username, &opt.password))?;
    let peer = opt.peer;

    let client = track!(fibers_global::execute(UdpClient::allocate(
        server_addr,
        auth_params
    )))?;
    eprintln!("# ALLOCATED: server={:?}", server_addr);

    eprintln!("Relay address is {:?}", client.relay_addr());

    let use_channel_data = opt.use_channel_data;
    let client = track!(fibers_global::execute(
        wait(client, move |client| if use_channel_data {
            client.channel_bind(peer)
        } else {
            client.create_permission(peer)
        })
        .and_then(|(client, result)| result.map(move |_| client))
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
    }))?;

    Ok(())
}

struct SendRecvLoop {
    peer: SocketAddr,
    client: UdpClient,
    stdin: fibers::io::Stdin,
}
impl Future for SendRecvLoop {
    type Item = ();
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        while let Async::Ready(Some((peer, data))) = track!(self.client.poll_recv())? {
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
                track!(self.client.start_send(self.peer, data))?;
            }
        }

        track!(self.client.poll_send())?;
        Ok(Async::NotReady)
    }
}
