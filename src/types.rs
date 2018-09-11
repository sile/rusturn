use std::net::SocketAddr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransportProtocol {
    Udp,
    Tcp,
    Tls,
}
impl TransportProtocol {
    pub fn as_u8(&self) -> u8 {
        match self {
            TransportProtocol::Udp => 17,
            TransportProtocol::Tcp => 6,
            _ => panic!("TODO"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FiveTuple {
    pub client: SocketAddr,
    pub server: SocketAddr,
    pub protocol: TransportProtocol,
}
