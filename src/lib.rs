#[macro_use]
extern crate bytecodec;
extern crate factory;
extern crate fibers;
#[cfg(test)]
extern crate fibers_global;
extern crate fibers_timeout_queue;
extern crate fibers_transport;
extern crate futures;
extern crate rand;
extern crate rustun;
#[macro_use]
extern crate stun_codec;
#[macro_use]
extern crate trackable;

pub use rustun::{Error, ErrorKind, Result};

pub mod attribute;

pub mod auth;
pub mod client;
pub mod server;
pub mod transport;

mod channel_data;
mod turn_message;

#[derive(Debug)]
pub struct AsyncResult<T>(fibers::sync::oneshot::Monitor<T, Error>);
impl<T> AsyncResult<T> {
    fn new() -> (Self, AsyncReply<T>) {
        let (tx, rx) = fibers::sync::oneshot::monitor();
        (AsyncResult(rx), AsyncReply(tx))
    }
}
impl<T> futures::Future for AsyncResult<T> {
    type Item = T;
    type Error = Error;

    fn poll(&mut self) -> futures::Poll<Self::Item, Self::Error> {
        track!(self.0.poll().map_err(Error::from))
    }
}

#[derive(Debug)]
struct AsyncReply<T>(fibers::sync::oneshot::Monitored<T, Error>);
impl<T> AsyncReply<T> {
    fn send(self, result: Result<T>) {
        let _ = self.0.exit(result);
    }
}

#[cfg(test)]
mod tests {
    use futures::Future;
    use rustun;
    use rustun::message::Request;
    use rustun::transport::StunUdpTransporter;
    use stun_codec::rfc5389;
    use stun_codec::{MessageDecoder, MessageEncoder};
    use trackable::error::MainError;

    use super::*;
    use auth::AuthParams;
    use transport::UdpOverTurnTransporter;

    #[test]
    fn it_works() -> std::result::Result<(), MainError> {
        let auth_params = track!(AuthParams::new("foo".to_owned(), "bar".to_owned()))?;

        // STUN server (peer)
        let stun_server = fibers_global::execute(rustun::server::UdpServer::start(
            fibers_global::handle(),
            "127.0.0.1:0".parse().unwrap(),
            rustun::server::BindingHandler,
        ))?;
        let stun_server_addr = stun_server.local_addr();
        fibers_global::spawn(stun_server.map(|_| ()).map_err(|e| panic!("{}", e)));

        // TURN server
        let turn_server =
            fibers_global::execute(server::UdpServer::start("127.0.0.1:0".parse().unwrap()))?;
        let turn_server_addr = turn_server.local_addr();
        fibers_global::spawn(turn_server.map(|_| ()).map_err(|e| panic!("{}", e)));

        // TURN client
        let turn_client = track!(fibers_global::execute(client::UdpClient::allocate(
            turn_server_addr,
            auth_params
        )))?;
        let transporter =
            UdpOverTurnTransporter::<_, MessageEncoder<_>, MessageDecoder<_>>::new(turn_client);

        // STUN client (over TURN)
        let stun_channel = rustun::channel::Channel::new(StunUdpTransporter::new(transporter));
        let stun_client = rustun::client::Client::new(&fibers_global::handle(), stun_channel);

        // BINDING request
        let request = Request::<rfc5389::Attribute>::new(rfc5389::methods::BINDING);
        let response = track!(fibers_global::execute(
            stun_client.call(stun_server_addr, request)
        ))?;
        assert!(response.is_ok(), "{:?}", response);

        Ok(())
    }
}
