extern crate fibers_global;
extern crate futures;
extern crate rusturn;
#[macro_use]
extern crate trackable;

use futures::{Future, Stream};
//use rusturn::client::Client;
use rusturn::auth::AuthParams;
use rusturn::client2::{Client, UdpClient};

fn main() -> Result<(), trackable::error::MainError> {
    let server_addr = track_any_err!("127.0.0.1:3478".parse())?;
    let auth_params = track!(AuthParams::new("foo".to_owned(), "bar".to_owned()))?;
    let mut client = track!(fibers_global::execute(UdpClient::allocate(
        server_addr,
        auth_params
    )))?;

    // let mut client = track!(fibers_global::execute(Client::udp_allocate(
    //     server_addr,
    //     "foo".to_owned(),
    //     "bar".to_owned()
    // )))?;

    // let peer = "127.0.0.1:2000".parse().unwrap();
    // // client.create_permission(peer)?;
    // // client.send(peer, vec![b'f', b'o', b'o'])?;

    // client.channel_bind(peer)?;

    // fibers_global::execute(client.for_each(|m| {
    //     println!("# M: {:?}", m);
    //     Ok(())
    // }))?;

    Ok(())
}
