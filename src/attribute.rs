use rustun::Attribute as StunAttribute;
use rustun::message::RawMessage;
use rustun::attribute::{Type, RawAttribute};
use rustun::types::TryAsRef;
use rustun::rfc5389;
use trackable::error::ErrorKindExt;

use {Result, ErrorKind};
use rfc5766;

macro_rules! impl_attr_from {
    ($rfc:ident, $attr:ident) => {
        impl From<$rfc::attributes::$attr> for Attribute {
            fn from(f: $rfc::attributes::$attr) -> Self {
                Attribute::$attr(f)
            }
        }
    }
}
macro_rules! impl_attr_try_as_ref {
    ($rfc:ident, $attr:ident) => {
        impl TryAsRef<$rfc::attributes::$attr> for Attribute {
            fn try_as_ref(&self) -> Option<& $rfc::attributes::$attr> {
                if let Attribute::$attr(ref a) = *self {
                    Some(a)
                } else {
                    None
                }
            }
        }
    }
}

/// Attribute set that are used in [RFC 5766](https://tools.ietf.org/html/rfc5766).
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Attribute {
    MappedAddress(rfc5389::attributes::MappedAddress),
    Username(rfc5389::attributes::Username),
    MessageIntegrity(rfc5389::attributes::MessageIntegrity),
    ErrorCode(rfc5389::attributes::ErrorCode),
    UnknownAttributes(rfc5389::attributes::UnknownAttributes),
    Realm(rfc5389::attributes::Realm),
    Nonce(rfc5389::attributes::Nonce),
    XorMappedAddress(rfc5389::attributes::XorMappedAddress),
    Software(rfc5389::attributes::Software),
    AlternateServer(rfc5389::attributes::AlternateServer),
    Fingerprint(rfc5389::attributes::Fingerprint),
    ChannelNumber(rfc5766::attributes::ChannelNumber),
    Lifetime(rfc5766::attributes::Lifetime),
    XorPeerAddress(rfc5766::attributes::XorPeerAddress),
    Data(rfc5766::attributes::Data),
    XorRelayedAddress(rfc5766::attributes::XorRelayedAddress),
    EvenPort(rfc5766::attributes::EvenPort),
    RequestedTransport(rfc5766::attributes::RequestedTransport),
    DontFragment(rfc5766::attributes::DontFragment),
    ReservationToken(rfc5766::attributes::ReservationToken),
}
impl_attr_from!(rfc5389, MappedAddress);
impl_attr_from!(rfc5389, Username);
impl_attr_from!(rfc5389, MessageIntegrity);
impl_attr_from!(rfc5389, ErrorCode);
impl_attr_from!(rfc5389, UnknownAttributes);
impl_attr_from!(rfc5389, Realm);
impl_attr_from!(rfc5389, Nonce);
impl_attr_from!(rfc5389, XorMappedAddress);
impl_attr_from!(rfc5389, Software);
impl_attr_from!(rfc5389, AlternateServer);
impl_attr_from!(rfc5389, Fingerprint);
impl_attr_from!(rfc5766, ChannelNumber);
impl_attr_from!(rfc5766, Lifetime);
impl_attr_from!(rfc5766, XorPeerAddress);
impl_attr_from!(rfc5766, Data);
impl_attr_from!(rfc5766, XorRelayedAddress);
impl_attr_from!(rfc5766, EvenPort);
impl_attr_from!(rfc5766, RequestedTransport);
impl_attr_from!(rfc5766, DontFragment);
impl_attr_from!(rfc5766, ReservationToken);
impl_attr_try_as_ref!(rfc5389, MappedAddress);
impl_attr_try_as_ref!(rfc5389, Username);
impl_attr_try_as_ref!(rfc5389, MessageIntegrity);
impl_attr_try_as_ref!(rfc5389, ErrorCode);
impl_attr_try_as_ref!(rfc5389, UnknownAttributes);
impl_attr_try_as_ref!(rfc5389, Realm);
impl_attr_try_as_ref!(rfc5389, Nonce);
impl_attr_try_as_ref!(rfc5389, XorMappedAddress);
impl_attr_try_as_ref!(rfc5389, Software);
impl_attr_try_as_ref!(rfc5389, AlternateServer);
impl_attr_try_as_ref!(rfc5389, Fingerprint);
impl_attr_try_as_ref!(rfc5766, ChannelNumber);
impl_attr_try_as_ref!(rfc5766, Lifetime);
impl_attr_try_as_ref!(rfc5766, XorPeerAddress);
impl_attr_try_as_ref!(rfc5766, Data);
impl_attr_try_as_ref!(rfc5766, XorRelayedAddress);
impl_attr_try_as_ref!(rfc5766, EvenPort);
impl_attr_try_as_ref!(rfc5766, RequestedTransport);
impl_attr_try_as_ref!(rfc5766, DontFragment);
impl_attr_try_as_ref!(rfc5766, ReservationToken);
impl StunAttribute for Attribute {
    fn get_type(&self) -> Type {
        match *self {
            Attribute::MappedAddress(ref a) => a.get_type(),
            Attribute::Username(ref a) => a.get_type(),
            Attribute::MessageIntegrity(ref a) => a.get_type(),
            Attribute::ErrorCode(ref a) => a.get_type(),
            Attribute::UnknownAttributes(ref a) => a.get_type(),
            Attribute::Realm(ref a) => a.get_type(),
            Attribute::Nonce(ref a) => a.get_type(),
            Attribute::XorMappedAddress(ref a) => a.get_type(),
            Attribute::Software(ref a) => a.get_type(),
            Attribute::AlternateServer(ref a) => a.get_type(),
            Attribute::Fingerprint(ref a) => a.get_type(),
            Attribute::ChannelNumber(ref a) => a.get_type(),
            Attribute::Lifetime(ref a) => a.get_type(),
            Attribute::XorPeerAddress(ref a) => a.get_type(),
            Attribute::Data(ref a) => a.get_type(),
            Attribute::XorRelayedAddress(ref a) => a.get_type(),
            Attribute::EvenPort(ref a) => a.get_type(),
            Attribute::RequestedTransport(ref a) => a.get_type(),
            Attribute::DontFragment(ref a) => a.get_type(),
            Attribute::ReservationToken(ref a) => a.get_type(),
        }
    }
    fn try_from_raw(attr: &RawAttribute, message: &RawMessage) -> Result<Self> {
        match attr.get_type().as_u16() {
            rfc5389::attributes::TYPE_MAPPED_ADDRESS => {
                rfc5389::attributes::MappedAddress::try_from_raw(attr, message).map(From::from)
            }
            rfc5389::attributes::TYPE_USERNAME => {
                rfc5389::attributes::Username::try_from_raw(attr, message).map(From::from)
            }
            rfc5389::attributes::TYPE_MESSAGE_INTEGRITY => {
                rfc5389::attributes::MessageIntegrity::try_from_raw(attr, message).map(From::from)
            }
            rfc5389::attributes::TYPE_ERROR_CODE => {
                rfc5389::attributes::ErrorCode::try_from_raw(attr, message).map(From::from)
            }
            rfc5389::attributes::TYPE_UNKNOWN_ATTRIBUTES => {
                rfc5389::attributes::UnknownAttributes::try_from_raw(attr, message).map(From::from)
            }
            rfc5389::attributes::TYPE_REALM => {
                rfc5389::attributes::Realm::try_from_raw(attr, message).map(From::from)
            }
            rfc5389::attributes::TYPE_NONCE => {
                rfc5389::attributes::Nonce::try_from_raw(attr, message).map(From::from)
            }
            rfc5389::attributes::TYPE_XOR_MAPPED_ADDRESS => {
                rfc5389::attributes::XorMappedAddress::try_from_raw(attr, message).map(From::from)
            }
            rfc5389::attributes::TYPE_SOFTWARE => {
                rfc5389::attributes::Software::try_from_raw(attr, message).map(From::from)
            }
            rfc5389::attributes::TYPE_ALTERNATE_SERVER => {
                rfc5389::attributes::AlternateServer::try_from_raw(attr, message).map(From::from)
            }
            rfc5389::attributes::TYPE_FINGERPRINT => {
                rfc5389::attributes::Fingerprint::try_from_raw(attr, message).map(From::from)
            }
            rfc5766::attributes::TYPE_CHANNEL_NUMBER => {
                rfc5766::attributes::ChannelNumber::try_from_raw(attr, message).map(From::from)
            }
            rfc5766::attributes::TYPE_LIFETIME => {
                rfc5766::attributes::Lifetime::try_from_raw(attr, message).map(From::from)
            }
            rfc5766::attributes::TYPE_XOR_PEER_ADDRESS => {
                rfc5766::attributes::XorPeerAddress::try_from_raw(attr, message).map(From::from)
            }
            rfc5766::attributes::TYPE_DATA => {
                rfc5766::attributes::Data::try_from_raw(attr, message).map(From::from)
            }
            rfc5766::attributes::TYPE_XOR_RELAYED_ADDRESS => {
                rfc5766::attributes::XorRelayedAddress::try_from_raw(attr, message).map(From::from)
            }
            rfc5766::attributes::TYPE_EVEN_PORT => {
                rfc5766::attributes::EvenPort::try_from_raw(attr, message).map(From::from)
            }
            rfc5766::attributes::TYPE_REQUESTED_TRANSPORT => {
                rfc5766::attributes::RequestedTransport::try_from_raw(attr, message).map(From::from)
            }
            rfc5766::attributes::TYPE_DONT_FRAGMENT => {
                rfc5766::attributes::DontFragment::try_from_raw(attr, message).map(From::from)
            }
            rfc5766::attributes::TYPE_RESERVATION_TOKEN => {
                rfc5766::attributes::ReservationToken::try_from_raw(attr, message).map(From::from)
            }
            t => Err(ErrorKind::Unsupported.cause(format!("Unknown attribute: type={}", t))),
        }
    }
    fn encode_value(&self, message: &RawMessage) -> Result<Vec<u8>> {
        match *self {
            Attribute::MappedAddress(ref a) => a.encode_value(message),
            Attribute::Username(ref a) => a.encode_value(message),
            Attribute::MessageIntegrity(ref a) => a.encode_value(message),
            Attribute::ErrorCode(ref a) => a.encode_value(message),
            Attribute::UnknownAttributes(ref a) => a.encode_value(message),
            Attribute::Realm(ref a) => a.encode_value(message),
            Attribute::Nonce(ref a) => a.encode_value(message),
            Attribute::XorMappedAddress(ref a) => a.encode_value(message),
            Attribute::Software(ref a) => a.encode_value(message),
            Attribute::AlternateServer(ref a) => a.encode_value(message),
            Attribute::Fingerprint(ref a) => a.encode_value(message),
            Attribute::ChannelNumber(ref a) => a.encode_value(message),
            Attribute::Lifetime(ref a) => a.encode_value(message),
            Attribute::XorPeerAddress(ref a) => a.encode_value(message),
            Attribute::Data(ref a) => a.encode_value(message),
            Attribute::XorRelayedAddress(ref a) => a.encode_value(message),
            Attribute::EvenPort(ref a) => a.encode_value(message),
            Attribute::RequestedTransport(ref a) => a.encode_value(message),
            Attribute::DontFragment(ref a) => a.encode_value(message),
            Attribute::ReservationToken(ref a) => a.encode_value(message),
        }
    }
}
