use crate::attribute::Attribute;
use crate::channel_data::{ChannelData, ChannelDataDecoder, ChannelDataEncoder};
use bytecodec::{ByteCount, Decode, Encode, EncodeExt, Eos, ErrorKind, Result, SizedEncode};
use stun_codec as stun;

#[derive(Debug)]
pub enum TurnMessage {
    Stun(stun::Message<Attribute>),
    BrokenStun(stun::BrokenMessage),
    ChannelData(ChannelData),
}

#[derive(Debug, Default)]
#[allow(clippy::large_enum_variant)]
pub enum TurnMessageDecoder {
    Stun(stun::MessageDecoder<Attribute>),
    ChannelData(ChannelDataDecoder),
    #[default]
    None,
}

impl Decode for TurnMessageDecoder {
    type Item = TurnMessage;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<usize> {
        loop {
            let next = match self {
                TurnMessageDecoder::Stun(x) => {
                    let result = track!(x.decode(buf, eos));
                    if result.is_err() {
                        *self = TurnMessageDecoder::None;
                    }
                    return result;
                }
                TurnMessageDecoder::ChannelData(x) => {
                    let result = track!(x.decode(buf, eos));
                    if result.is_err() {
                        *self = TurnMessageDecoder::None;
                    }
                    return result;
                }
                TurnMessageDecoder::None => match buf.first().map(|&b| b >> 6) {
                    None => return Ok(0),
                    Some(0b00) => TurnMessageDecoder::Stun(Default::default()),
                    Some(0b01) => TurnMessageDecoder::ChannelData(Default::default()),
                    Some(prefix) => {
                        track_panic!(
                            ErrorKind::InvalidInput,
                            "Unknown codec: prefix=0b{:b}",
                            prefix
                        );
                    }
                },
            };
            *self = next;
        }
    }

    fn finish_decoding(&mut self) -> Result<Self::Item> {
        let item = match self {
            TurnMessageDecoder::Stun(x) => track!(x.finish_decoding()).map(|x| {
                x.map(TurnMessage::Stun)
                    .unwrap_or_else(TurnMessage::BrokenStun)
            })?,
            TurnMessageDecoder::ChannelData(x) => {
                track!(x.finish_decoding().map(TurnMessage::ChannelData))?
            }
            TurnMessageDecoder::None => track_panic!(ErrorKind::IncompleteDecoding),
        };
        *self = TurnMessageDecoder::None;
        Ok(item)
    }

    fn requiring_bytes(&self) -> ByteCount {
        match self {
            TurnMessageDecoder::Stun(x) => x.requiring_bytes(),
            TurnMessageDecoder::ChannelData(x) => x.requiring_bytes(),
            TurnMessageDecoder::None => ByteCount::Finite(0),
        }
    }
}

#[derive(Debug, Default)]
#[allow(clippy::large_enum_variant)]
pub enum TurnMessageEncoder {
    Stun(stun::MessageEncoder<Attribute>),
    ChannelData(ChannelDataEncoder),
    #[default]
    None,
}
impl Encode for TurnMessageEncoder {
    type Item = TurnMessage;

    fn encode(&mut self, buf: &mut [u8], eos: Eos) -> Result<usize> {
        match self {
            TurnMessageEncoder::Stun(x) => track!(x.encode(buf, eos)),
            TurnMessageEncoder::ChannelData(x) => track!(x.encode(buf, eos)),
            TurnMessageEncoder::None => Ok(0),
        }
    }

    fn start_encoding(&mut self, item: Self::Item) -> Result<()> {
        track_assert!(self.is_idle(), ErrorKind::EncoderFull);
        *self = match item {
            TurnMessage::Stun(t) => track!(EncodeExt::with_item(t).map(TurnMessageEncoder::Stun))?,
            TurnMessage::BrokenStun(t) => {
                track_panic!(ErrorKind::InvalidInput, "{:?}", t);
            }
            TurnMessage::ChannelData(t) => {
                track!(EncodeExt::with_item(t).map(TurnMessageEncoder::ChannelData))?
            }
        };
        Ok(())
    }

    fn requiring_bytes(&self) -> ByteCount {
        ByteCount::Finite(self.exact_requiring_bytes())
    }
}
impl SizedEncode for TurnMessageEncoder {
    fn exact_requiring_bytes(&self) -> u64 {
        match self {
            TurnMessageEncoder::Stun(x) => x.exact_requiring_bytes(),
            TurnMessageEncoder::ChannelData(x) => x.exact_requiring_bytes(),
            TurnMessageEncoder::None => 0,
        }
    }
}
