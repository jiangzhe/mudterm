use crate::error::{Error, Result};
use encoding::codec::simpchinese::GB18030_ENCODING;
use encoding::codec::tradchinese::BigFive2003Encoding;
use encoding::codec::utf_8::UTF8Decoder;
use encoding::types::{CodecError, RawDecoder};
use encoding::{EncoderTrap, Encoding};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Codec {
    Gb18030,
    Utf8,
    Big5,
}

impl Default for Codec {
    fn default() -> Self {
        Codec::Gb18030
    }
}

pub struct Decoder(Box<dyn RawDecoder>);

impl Default for Decoder {
    fn default() -> Self {
        Self(GB18030_ENCODING.raw_decoder())
    }
}

impl Decoder {
    pub fn decode_raw_to(
        &mut self,
        input: impl AsRef<[u8]>,
        output: &mut String,
    ) -> (usize, Option<CodecError>) {
        self.0.raw_feed(input.as_ref(), output)
    }

    pub fn switch_codec(&mut self, code: Codec) {
        match code {
            Codec::Gb18030 => self.0 = GB18030_ENCODING.raw_decoder(),
            Codec::Utf8 => self.0 = UTF8Decoder::new(),
            Codec::Big5 => self.0 = BigFive2003Encoding.raw_decoder(),
        }
    }
}

pub struct Encoder(Codec);

impl Default for Encoder {
    fn default() -> Self {
        Self(Codec::Gb18030)
    }
}

impl Encoder {
    pub fn encode_to(&self, input: impl AsRef<str>, output: &mut Vec<u8>) -> Result<()> {
        let input = input.as_ref();
        match self.0 {
            Codec::Gb18030 => GB18030_ENCODING
                .encode_to(input, EncoderTrap::Strict, output)
                .map_err(|e| Error::EncodeError(format!("\"{}\"", e)))?,
            Codec::Utf8 => output.extend_from_slice(input.as_bytes()),
            Codec::Big5 => BigFive2003Encoding
                .encode_to(input, EncoderTrap::Strict, output)
                .map_err(|e| Error::EncodeError(format!("\"{}\"", e)))?,
        }
        Ok(())
    }

    pub fn switch_codec(&mut self, code: Codec) {
        self.0 = code;
    }
}

pub struct MudCodec {
    gcodec: Codec,
    decoder: Decoder,
    encoder: Encoder,
}

impl MudCodec {
    pub fn new() -> Self {
        Self {
            gcodec: Codec::default(),
            decoder: Decoder::default(),
            encoder: Encoder::default(),
        }
    }

    pub fn switch_codec(&mut self, code: Codec) {
        self.gcodec = code;
        self.decoder.switch_codec(code);
        self.encoder.switch_codec(code);
    }

    pub fn decode(&mut self, bs: &[u8]) -> String {
        let mut s = String::new();
        let _ = self.decoder.decode_raw_to(bs, &mut s);
        s
    }

    pub fn encode(&self, s: &str) -> Result<Vec<u8>> {
        let mut output = Vec::new();
        self.encoder.encode_to(s, &mut output)?;
        Ok(output)
    }

    pub fn encoder(&self) -> &Encoder {
        &self.encoder
    }

    pub fn decoder(&mut self) -> &mut Decoder {
        &mut self.decoder
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_slice_rposition() {
        let bs = vec![0u8, 0, 0, 1, 0, 0];
        let pos = bs.iter().rposition(|&b| b == 1);
        println!("pos={:?}", pos);
        assert_eq!(3, pos.unwrap());
    }

    #[test]
    fn test_gb18030() {
        let mut mc = MudCodec::new();
        mc.switch_codec(Codec::Gb18030);
        let bs = b"\xc4\xe3\xcf\xd6\xd4\xda\xb2\xbb\xc3\xa6\x0a".to_vec();
        let s = mc.decode(&bs);
        assert_eq!(&s[..], "你现在不忙\n");
    }
}
