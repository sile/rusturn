use bytecodec::{Decode, Encode};
use fibers::sync::mpsc;
use futures::{Async, Stream};
use rustun::transport::{StunTransport, Transport, UnreliableTransport};
use std::net::SocketAddr;
use stun_codec::{Message, MessageDecoder, MessageEncoder};

use attribute::Attribute;
use channel_data::{ChannelData, ChannelDataDecoder, ChannelDataEncoder};
use Result;

pub struct Relayer<T: Transport> {
    send_rx: mpsc::Receiver<(SocketAddr, <T::Encoder as Encode>::Item)>,
    recv_tx: mpsc::Sender<(SocketAddr, <T::Decoder as Decode>::Item)>,
}

pub struct TransporterHandle<T: Transport> {
    send_tx: mpsc::Sender<(SocketAddr, <T::Encoder as Encode>::Item)>,
    recv_rx: mpsc::Receiver<(SocketAddr, <T::Decoder as Decode>::Item)>,
}
impl<T: Transport> TransporterHandle<T> {
    pub fn new() -> (Self, Relayer<T>) {
        let (send_tx, send_rx) = mpsc::channel();
        let (recv_tx, recv_rx) = mpsc::channel();
        let this = TransporterHandle { send_tx, recv_rx };
        let relayer = Relayer { send_rx, recv_tx };
        (this, relayer)
    }
}
impl<T: Transport> Transport for TransporterHandle<T> {
    type Decoder = T::Decoder;
    type Encoder = T::Encoder;

    fn send(&mut self, peer: SocketAddr, message: <Self::Encoder as Encode>::Item) {
        let _ = self.send_tx.send((peer, message)); // TODO
    }

    fn recv(&mut self) -> Option<(SocketAddr, <Self::Decoder as Decode>::Item)> {
        match self.recv_rx.poll() {
            Err(()) => panic!("TODO"),
            Ok(Async::NotReady) => None,
            Ok(Async::Ready(None)) => panic!("TODO"),
            Ok(Async::Ready(Some(item))) => Some(item),
        }
    }

    fn run_once(&mut self) -> Result<bool> {
        panic!()
    }
}
impl<T: UnreliableTransport> UnreliableTransport for TransporterHandle<T> {}

// #[derive(Debug)]
// pub struct TurnTransporter {}
// impl Transport for TurnTransporter {
//     type Decoder = TurnMessageDecoder;
//     type Encoder = TurnMessageEncoder;
// }

// #[derive(Debug)]
// pub enum TurnMessage {
//     Stun(Message<Attribute>),
//     ChannelData(ChannelData),
// }

// #[derive(Debug, Default)]
// pub struct TurnMessageDecoder;

// #[derive(Debug, Default)]
// pub struct TurnMessageEncoder;
pub struct StunTransporter {}
// impl Transport for StunTransporter {
//     type Decoder = MessageDecoder<Attribute>;
//     type Encoder = MessageEncoder<Attribute>;

//     fn send(&mut self, peer: SocketAddr, message: <Self::Encoder as Encode>::Item) {
//         panic!()
//     }

//     fn recv(&mut self) -> Option<(SocketAddr, <Self::Decoder as Decode>::Item)> {
//         panic!()
//     }

//     fn run_once(&mut self) -> Result<bool> {
//         panic!()
//     }
// }

pub struct DataChannelTransporter {}
