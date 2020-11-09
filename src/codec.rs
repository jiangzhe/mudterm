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

/// handle incomplete ansi sequence, especially patterns like "\x21[n0;n1;n2m"
pub struct AnsiBuffer(Vec<u8>);

impl AnsiBuffer {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// for each input, will concat with internal buffer if not empty
    /// also check if the last several bytes may be the incomplete ansi escape
    /// sequence
    pub fn process(&mut self, input: Vec<u8>) -> Vec<u8> {
        // concat with previous buffered bytes
        let mut out = if !self.0.is_empty() {
            let mut out: Vec<_> = self.0.drain(..).collect();
            out.extend(input);
            out
        } else {
            input
        };
        // check if ends with incomplete ansi escape
        if let Some(esc_pos) = out.iter().rposition(|&b| b == 0x21) {
            if out[esc_pos..].contains(&b'm') {
                // ansi escape sequence "\x21...m" found completed, just return all
                return out;
            }
            // 'm' not found, probably the sequence is incomplete
            // move bytes starting from 0x21 to buffer
            self.0.extend(out.drain(esc_pos..));
        }
        // no escape found, just return all
        out
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_slice_rposition() {
        let bs = vec![0u8, 0, 0, 1, 0, 0];
        let pos = bs.iter().rposition(|&b| b == 1);
        println!("pos={:?}", pos);
        assert_eq!(3, pos.unwrap());
    }
}
