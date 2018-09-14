use futures::{Async, Future, Poll, Stream};
use rustun::channel::Channel as StunChannel;
use rustun::message::{Request, Response};
use rustun::transport::{StunTransport, Transport};
use std::net::SocketAddr;
use stun_codec::{rfc5389, rfc5766};

use super::core::ClientCore;
use super::StunTransaction;
use attribute::Attribute;
use auth::AuthParams;
use channel_data::ChannelData;
use {Error, ErrorKind, Result};

const TRANSPORT_PROTOCOL_UDP: u8 = 17;

#[derive(Debug)]
pub struct Allocate<S, C> {
    server_addr: SocketAddr,
    stun_channel: Option<StunChannel<Attribute, S>>,
    channel_data_transporter: Option<C>,
    auth_params: AuthParams,
    allocate_transaction: Option<StunTransaction>,
}
impl<S, C> Allocate<S, C>
where
    S: StunTransport<Attribute> + 'static,
    C: Transport<SendItem = ChannelData, RecvItem = ChannelData>,
{
    pub fn new(
        server_addr: SocketAddr,
        stun_channel: StunChannel<Attribute, S>,
        channel_data_transporter: C,
        auth_params: AuthParams,
    ) -> Self {
        Allocate {
            server_addr,
            stun_channel: Some(stun_channel),
            channel_data_transporter: Some(channel_data_transporter),
            auth_params,
            allocate_transaction: None,
        }
    }

    fn start_allocate(&mut self) -> Result<()> {
        let mut request = Request::new(rfc5766::methods::ALLOCATE);

        let requested_transport =
            rfc5766::attributes::RequestedTransport::new(TRANSPORT_PROTOCOL_UDP).into();
        request.add_attribute(requested_transport);

        if self.auth_params.has_realm() {
            track!(self.auth_params.add_auth_attributes(&mut request))?;
        }
        self.allocate_transaction = Some(StunTransaction::new(
            self.stun_channel
                .as_mut()
                .expect("never fails")
                .call(self.server_addr, request),
        ));
        Ok(())
    }

    fn handle_allocate_response(
        &mut self,
        response: Response<Attribute>,
    ) -> Result<Option<ClientCore<S, C>>> {
        match response {
            Ok(response) => {
                let mut lifetime = None;
                for attr in response.attributes() {
                    match attr {
                        Attribute::Lifetime(a) => {
                            lifetime = Some(a.lifetime());
                        }
                        Attribute::MessageIntegrity(a) => {
                            track!(self.auth_params.validate(&a))?;
                        }
                        _ => {}
                    }
                }

                let lifetime = track_assert_some!(lifetime, ErrorKind::Other; response);
                let client = ClientCore::new(
                    self.server_addr,
                    self.stun_channel.take().expect("never fails"),
                    self.channel_data_transporter.take().expect("never fails"),
                    self.auth_params.clone(),
                    lifetime,
                );
                Ok(Some(client))
            }
            Err(response) => {
                track_assert!(!self.auth_params.has_realm(), ErrorKind::Other; response);

                for attr in response.attributes() {
                    match attr {
                        Attribute::ErrorCode(e) => {
                            track_assert_eq!(e.code(), rfc5389::errors::Unauthorized::CODEPOINT,
                                             ErrorKind::Other; response);
                        }
                        Attribute::Realm(a) => {
                            self.auth_params.set_realm(a.clone());
                        }
                        Attribute::Nonce(a) => {
                            self.auth_params.set_nonce(a.clone());
                        }
                        _ => {}
                    }
                }
                track_assert!(self.auth_params.has_realm(), ErrorKind::Other; response);
                track_assert!(self.auth_params.has_nonce(), ErrorKind::Other; response);

                track!(self.start_allocate())?;
                Ok(None)
            }
        }
    }

    fn stun_channel_mut(&mut self) -> &mut StunChannel<Attribute, S> {
        self.stun_channel
            .as_mut()
            .expect("Cannot poll Allocate twice")
    }

    fn channel_data_transporter_mut(&mut self) -> &mut C {
        self.channel_data_transporter
            .as_mut()
            .expect("Cannot poll Allocate twice")
    }
}
impl<S, C> Future for Allocate<S, C>
where
    S: StunTransport<Attribute> + 'static,
    C: Transport<SendItem = ChannelData, RecvItem = ChannelData>,
{
    type Item = ClientCore<S, C>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let mut did_something = true;
        while did_something {
            did_something = false;

            if self.allocate_transaction.is_none() {
                did_something = true;
                track!(self.start_allocate())?;
            }

            if let Async::Ready(Some(message)) = track!(self.stun_channel_mut().poll())? {
                track_panic!(
                    ErrorKind::Other,
                    "Unexpected message reception: {:?}",
                    message
                );
            }
            if let Some(data) = self.channel_data_transporter_mut().recv() {
                track_panic!(ErrorKind::Other, "Unexpected data reception: {:?}", data);
            }
            if track!(self.channel_data_transporter_mut().run_once())? {
                track_panic!(ErrorKind::Other, "Unexpected termination");
            }

            if let Async::Ready(Some(response)) = track!(self.allocate_transaction.poll())? {
                did_something = true;
                if let Some(client) = track!(self.handle_allocate_response(response))? {
                    return Ok(Async::Ready(client));
                }
            }
        }
        Ok(Async::NotReady)
    }
}
