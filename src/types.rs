use std::net::SocketAddr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransportProtocol {
    Udp,
    Tcp,
    Tls,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FiveTuple {
    pub client: SocketAddr,
    pub server: SocketAddr,
    pub protocol: TransportProtocol,
}
