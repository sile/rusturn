use handy_async::sync_io::{ReadExt, WriteExt};
use rustun::attribute::{RawAttribute, Type};
use rustun::message::RawMessage;
use rustun::types::SocketAddrValue;
use rustun::Attribute;
use std::net::SocketAddr;
use std::ops::Deref;
use std::time::Duration;

use {ErrorKind, Result};

pub const TYPE_CHANNEL_NUMBER: u16 = 0x000C;
pub const TYPE_LIFETIME: u16 = 0x000D;
pub const TYPE_XOR_PEER_ADDRESS: u16 = 0x0012;
pub const TYPE_DATA: u16 = 0x0013;
pub const TYPE_XOR_RELAYED_ADDRESS: u16 = 0x0016;
pub const TYPE_EVEN_PORT: u16 = 0x0018;
pub const TYPE_REQUESTED_TRANSPORT: u16 = 0x0019;
pub const TYPE_DONT_FRAGMENT: u16 = 0x001A;
pub const TYPE_RESERVATION_TOKEN: u16 = 0x0022;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChannelNumber(u16);
impl ChannelNumber {
    pub fn new(number: u16) -> Self {
        ChannelNumber(number)
    }
    pub fn number(&self) -> u16 {
        self.0
    }
}
impl Attribute for ChannelNumber {
    fn get_type(&self) -> Type {
        Type::new(TYPE_CHANNEL_NUMBER)
    }
    fn try_from_raw(attr: &RawAttribute, _message: &RawMessage) -> Result<Self> {
        track_assert_eq!(
            attr.get_type().as_u16(),
            TYPE_CHANNEL_NUMBER,
            ErrorKind::Unsupported
        );
        let number = track_try!((&mut attr.value()).read_u16be());
        Ok(Self::new(number))
    }
    fn encode_value(&self, _message: &RawMessage) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        track_try!((&mut buf).write_u16be(self.0));
        Ok(buf)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Lifetime(Duration);
impl Lifetime {
    pub fn new(duration: Duration) -> Self {
        Lifetime(duration)
    }
    pub fn duration(&self) -> Duration {
        self.0
    }
}
impl Attribute for Lifetime {
    fn get_type(&self) -> Type {
        Type::new(TYPE_LIFETIME)
    }
    fn try_from_raw(attr: &RawAttribute, _message: &RawMessage) -> Result<Self> {
        track_assert_eq!(
            attr.get_type().as_u16(),
            TYPE_LIFETIME,
            ErrorKind::Unsupported
        );
        let seconds = track_try!((&mut attr.value()).read_u32be());
        Ok(Self::new(Duration::from_secs(seconds as u64)))
    }
    fn encode_value(&self, _message: &RawMessage) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        track_try!((&mut buf).write_u32be(self.0.as_secs() as u32));
        Ok(buf)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct XorPeerAddress(SocketAddr);
impl XorPeerAddress {
    pub fn new(addr: SocketAddr) -> Self {
        XorPeerAddress(addr)
    }
    pub fn address(&self) -> SocketAddr {
        self.0
    }
}
impl Attribute for XorPeerAddress {
    fn get_type(&self) -> Type {
        Type::new(TYPE_XOR_PEER_ADDRESS)
    }
    fn try_from_raw(attr: &RawAttribute, message: &RawMessage) -> Result<Self> {
        track_assert_eq!(
            attr.get_type().as_u16(),
            TYPE_XOR_PEER_ADDRESS,
            ErrorKind::Unsupported
        );
        let xor_addr = track_try!(SocketAddrValue::read_from(&mut attr.value()));
        Ok(Self::new(xor_addr.xor(message.transaction_id()).address()))
    }
    fn encode_value(&self, message: &RawMessage) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        let xor_addr = SocketAddrValue::new(self.0).xor(message.transaction_id());
        track_try!(xor_addr.write_to(&mut buf));
        Ok(buf)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Data(Vec<u8>);
impl Data {
    pub fn new(data: Vec<u8>) -> Self {
        Data(data)
    }
    pub fn unwrap(self) -> Vec<u8> {
        self.0
    }
}
impl Deref for Data {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl Attribute for Data {
    fn get_type(&self) -> Type {
        Type::new(TYPE_DATA)
    }
    fn try_from_raw(attr: &RawAttribute, _message: &RawMessage) -> Result<Self> {
        track_assert_eq!(attr.get_type().as_u16(), TYPE_DATA, ErrorKind::Unsupported);
        Ok(Self::new(Vec::from(attr.value())))
    }
    fn encode_value(&self, _message: &RawMessage) -> Result<Vec<u8>> {
        Ok(self.0.clone())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct XorRelayedAddress(SocketAddr);
impl XorRelayedAddress {
    pub fn new(addr: SocketAddr) -> Self {
        XorRelayedAddress(addr)
    }
    pub fn address(&self) -> SocketAddr {
        self.0
    }
}
impl Attribute for XorRelayedAddress {
    fn get_type(&self) -> Type {
        Type::new(TYPE_XOR_RELAYED_ADDRESS)
    }
    fn try_from_raw(attr: &RawAttribute, message: &RawMessage) -> Result<Self> {
        track_assert_eq!(
            attr.get_type().as_u16(),
            TYPE_XOR_RELAYED_ADDRESS,
            ErrorKind::Unsupported
        );
        let xor_addr = track_try!(SocketAddrValue::read_from(&mut attr.value()));
        Ok(Self::new(xor_addr.xor(message.transaction_id()).address()))
    }
    fn encode_value(&self, message: &RawMessage) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        let xor_addr = SocketAddrValue::new(self.0).xor(message.transaction_id());
        track_try!(xor_addr.write_to(&mut buf));
        Ok(buf)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EvenPort(bool);
impl EvenPort {
    pub fn new(is_even_port_requested: bool) -> Self {
        EvenPort(is_even_port_requested)
    }
    pub fn is_even_port_requested(&self) -> bool {
        self.0
    }
}
impl Attribute for EvenPort {
    fn get_type(&self) -> Type {
        Type::new(TYPE_EVEN_PORT)
    }
    fn try_from_raw(attr: &RawAttribute, _message: &RawMessage) -> Result<Self> {
        track_assert_eq!(
            attr.get_type().as_u16(),
            TYPE_EVEN_PORT,
            ErrorKind::Unsupported
        );
        let flags = track_try!((&mut attr.value()).read_u8());
        Ok(Self::new((flags & 0b1000_0000) != 0))
    }
    fn encode_value(&self, _message: &RawMessage) -> Result<Vec<u8>> {
        let flag = if self.is_even_port_requested() {
            0b1000_0000
        } else {
            0b0000_0000
        };
        Ok(vec![flag])
    }
}

const PROTOCOL_UDP: u8 = 17;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RequestedTransport(u8);
impl RequestedTransport {
    pub fn new() -> Self {
        RequestedTransport(PROTOCOL_UDP)
    }
    pub fn protocol(&self) -> u8 {
        self.0
    }
    pub fn is_udp(&self) -> bool {
        self.protocol() == PROTOCOL_UDP
    }
}
impl Attribute for RequestedTransport {
    fn get_type(&self) -> Type {
        Type::new(TYPE_REQUESTED_TRANSPORT)
    }
    fn try_from_raw(attr: &RawAttribute, _message: &RawMessage) -> Result<Self> {
        track_assert_eq!(
            attr.get_type().as_u16(),
            TYPE_REQUESTED_TRANSPORT,
            ErrorKind::Unsupported
        );
        let protocol = track_try!((&mut attr.value()).read_u8());
        track_assert_eq!(protocol, PROTOCOL_UDP, ErrorKind::Unsupported);
        Ok(Self::new())
    }
    fn encode_value(&self, _message: &RawMessage) -> Result<Vec<u8>> {
        let buf = vec![self.0, 0, 0, 0];
        Ok(buf)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DontFragment;
impl Attribute for DontFragment {
    fn get_type(&self) -> Type {
        Type::new(TYPE_DONT_FRAGMENT)
    }
    fn try_from_raw(attr: &RawAttribute, _message: &RawMessage) -> Result<Self> {
        track_assert_eq!(
            attr.get_type().as_u16(),
            TYPE_DONT_FRAGMENT,
            ErrorKind::Unsupported
        );
        Ok(DontFragment)
    }
    fn encode_value(&self, _message: &RawMessage) -> Result<Vec<u8>> {
        Ok(Vec::new())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReservationToken(u64);
impl ReservationToken {
    pub fn new(token: u64) -> Self {
        ReservationToken(token)
    }
    pub fn token(&self) -> u64 {
        self.0
    }
}
impl Attribute for ReservationToken {
    fn get_type(&self) -> Type {
        Type::new(TYPE_RESERVATION_TOKEN)
    }
    fn try_from_raw(attr: &RawAttribute, _message: &RawMessage) -> Result<Self> {
        track_assert_eq!(
            attr.get_type().as_u16(),
            TYPE_RESERVATION_TOKEN,
            ErrorKind::Unsupported
        );
        let token = track_try!((&mut attr.value()).read_u64be());
        Ok(Self::new(token))
    }
    fn encode_value(&self, _message: &RawMessage) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        track_try!((&mut buf).write_u64be(self.0));
        Ok(buf)
    }
}
