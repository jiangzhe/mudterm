use crate::error::{Error, Result};
use encoding::codec::simpchinese::GB18030_ENCODING;
use encoding::codec::tradchinese::BigFive2003Encoding;
use encoding::codec::utf_8::UTF8Decoder;
use encoding::types::{CodecError, RawDecoder, StringWriter};
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
        remove_carriage: bool,
    ) -> (usize, Option<CodecError>) {
        if remove_carriage {
            self.0
                .raw_feed(input.as_ref(), &mut RemoveCarriageWriter(output))
        } else {
            self.0.raw_feed(input.as_ref(), output)
        }
    }

    pub fn switch_codec(&mut self, code: Codec) {
        match code {
            Codec::Gb18030 => self.0 = GB18030_ENCODING.raw_decoder(),
            Codec::Utf8 => self.0 = UTF8Decoder::new(),
            Codec::Big5 => self.0 = BigFive2003Encoding.raw_decoder(),
        }
    }
}

struct RemoveCarriageWriter<'a>(&'a mut String);

impl<'a> StringWriter for RemoveCarriageWriter<'a> {
    #[inline]
    fn writer_hint(&mut self, _expectedlen: usize) {
        self.0.reserve(_expectedlen);
    }

    #[inline]
    fn write_char(&mut self, c: char) {
        if c != '\r' {
            self.0.write_char(c);
        }
    }

    #[inline]
    fn write_str(&mut self, s: &str) {
        for c in s.chars() {
            self.write_char(c);
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
    // ansi_buf: AnsiBuffer,
}

impl MudCodec {
    pub fn new() -> Self {
        Self {
            gcodec: Codec::default(),
            decoder: Decoder::default(),
            encoder: Encoder::default(),
            // ansi_buf: AnsiBuffer::new(),
        }
    }

    pub fn switch_codec(&mut self, code: Codec) {
        self.gcodec = code;
        self.decoder.switch_codec(code);
        self.encoder.switch_codec(code);
    }

    pub fn decode(&mut self, bs: &[u8], remove_carriage: bool) -> String {
        // let bs = self.ansi_buf.process(bs);
        let mut s = String::new();
        let _ = self.decoder.decode_raw_to(bs, &mut s, remove_carriage);
        s
    }

    pub fn encode(&mut self, s: &str) -> Result<Vec<u8>> {
        let mut output = Vec::new();
        self.encoder.encode_to(s, &mut output)?;
        Ok(output)
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
    fn test_incomplete_ansi_sequence() {
        let mut mc = MudCodec::new();
        mc.switch_codec(Codec::Utf8);
        let bs = b"hello\x21[37;".to_vec();
        let s = mc.decode(&bs, false);
        assert_eq!(&s[..], "hello");
        let bs = b"1mworld".to_vec();
        let s = mc.decode(&bs, false);
        assert_eq!(&s[..], "\x21[37;1mworld");
    }
}
