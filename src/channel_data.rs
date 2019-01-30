use bytecodec::bytes::{BytesDecoder, BytesEncoder};
use bytecodec::combinator::Peekable;
use bytecodec::fixnum::{U16beDecoder, U16beEncoder};
use bytecodec::{ByteCount, Decode, Encode, Eos, ErrorKind, Result, SizedEncode};
use stun_codec::rfc5766::attributes::ChannelNumber;

#[derive(Debug)]
pub struct ChannelData {
    channel_number: ChannelNumber,
    data: Vec<u8>,
}
impl ChannelData {
    pub fn new(channel_number: ChannelNumber, data: Vec<u8>) -> Result<Self> {
        track_assert!(data.len() <= 0xFFFF, ErrorKind::InvalidInput; data.len());
        Ok(ChannelData {
            channel_number,
            data,
        })
    }

    pub fn channel_number(&self) -> ChannelNumber {
        self.channel_number
    }

    pub fn into_data(self) -> Vec<u8> {
        self.data
    }
}

#[derive(Debug, Default)]
pub struct ChannelDataDecoder {
    channel_number: U16beDecoder,
    data_len: Peekable<U16beDecoder>,
    data: BytesDecoder,
}
impl Decode for ChannelDataDecoder {
    type Item = ChannelData;

    fn decode(&mut self, buf: &[u8], eos: Eos) -> Result<usize> {
        let mut offset = 0;
        bytecodec_try_decode!(self.channel_number, offset, buf, eos);
        if !self.data_len.is_idle() {
            bytecodec_try_decode!(self.data_len, offset, buf, eos);

            let len = self.data_len.peek().cloned().expect("never fails");
            self.data.set_bytes(vec![0; len as usize]);
        }
        bytecodec_try_decode!(self.data, offset, buf, eos);
        Ok(offset)
    }

    fn finish_decoding(&mut self) -> Result<Self::Item> {
        let channel_number = track!(self.channel_number.finish_decoding())?;
        let channel_number = track!(ChannelNumber::new(channel_number))?;
        let _ = track!(self.data_len.finish_decoding())?;
        let data = track!(self.data.finish_decoding())?;
        Ok(ChannelData {
            channel_number,
            data,
        })
    }

    fn requiring_bytes(&self) -> ByteCount {
        self.channel_number
            .requiring_bytes()
            .add_for_decoding(self.data_len.requiring_bytes())
            .add_for_decoding(self.data.requiring_bytes())
    }

    fn is_idle(&self) -> bool {
        self.channel_number.is_idle() && self.data_len.is_idle() && self.data.is_idle()
    }
}

#[derive(Debug, Default)]
pub struct ChannelDataEncoder {
    channel_number: U16beEncoder,
    data_len: U16beEncoder,
    data: BytesEncoder,
}
impl Encode for ChannelDataEncoder {
    type Item = ChannelData;

    fn encode(&mut self, buf: &mut [u8], eos: Eos) -> Result<usize> {
        let mut offset = 0;
        bytecodec_try_encode!(self.channel_number, offset, buf, eos);
        bytecodec_try_encode!(self.data_len, offset, buf, eos);
        bytecodec_try_encode!(self.data, offset, buf, eos);
        Ok(offset)
    }

    fn start_encoding(&mut self, item: Self::Item) -> Result<()> {
        track!(self
            .channel_number
            .start_encoding(item.channel_number.value()))?;
        track!(self.data_len.start_encoding(item.data.len() as u16))?;
        track!(self.data.start_encoding(item.data))?;
        Ok(())
    }

    fn requiring_bytes(&self) -> ByteCount {
        ByteCount::Finite(self.exact_requiring_bytes())
    }

    fn is_idle(&self) -> bool {
        self.channel_number.is_idle() && self.data_len.is_idle() && self.data.is_idle()
    }
}
impl SizedEncode for ChannelDataEncoder {
    fn exact_requiring_bytes(&self) -> u64 {
        self.channel_number.exact_requiring_bytes()
            + self.data_len.exact_requiring_bytes()
            + self.data.exact_requiring_bytes()
    }
}
