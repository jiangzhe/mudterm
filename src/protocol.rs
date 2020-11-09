use crate::error::{Error, Result};
use byteorder::{ReadBytesExt, WriteBytesExt, LE};
use std::io::Cursor;
use std::io::{Read, Write};
use tui::style::{Color, Modifier, Style};
use tui::text::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum Packet {
    Ok,
    AuthReq(Vec<u8>),
    AuthResp(Vec<u8>),
    Text(String),
    StyledText(Vec<Span<'static>>, bool),
    Err(String),
}

impl Packet {
    pub fn header(&self) -> u8 {
        match self {
            Self::Ok => 0x00,
            Self::AuthReq(_) => 0x01,
            Self::AuthResp(_) => 0x02,
            Self::Text(_) => 0x03,
            Self::StyledText(..) => 0x04,
            Self::Err(_) => 0xff,
        }
    }

    pub fn payload(self) -> Vec<u8> {
        match self {
            Self::Ok => vec![],
            Self::AuthReq(bs) | Self::AuthResp(bs) => bs,
            Self::Text(s) | Self::Err(s) => s.into_bytes(),
            Self::StyledText(spans, ended) => encode_styled_text(spans, ended).unwrap(),
        }
    }

    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let mut bs = Vec::new();
        loop {
            let len = read_packet(&mut reader, &mut bs)?;
            if len < 0xff_ffff {
                break;
            }
        }
        let pkt = match bs.pop().unwrap() {
            0x00 => Self::Ok,
            0x01 => Self::AuthReq(bs),
            0x02 => Self::AuthResp(bs),
            0x03 => Self::Text(String::from_utf8(bs)?),
            0x04 => {
                let (spans, ended) = decode_styled_text(&bs)?;
                Self::StyledText(spans, ended)
            }
            header => {
                return Err(Error::DecodeError(format!(
                    "invalid header in packet end {:x}",
                    header
                )))
            }
        };
        Ok(pkt)
    }

    pub fn write_to<W: Write>(self, mut writer: W) -> Result<()> {
        let header = self.header();
        let mut bs = self.payload();
        bs.push(header);
        let mut bs = &bs[..];
        // let mut bs = &self.0[..];
        while bs.len() >= 0xff_ffff {
            let (left, right) = bs.split_at(0xff_ffff);
            write_packet(&mut writer, left)?;
            bs = right;
        }
        write_packet(&mut writer, bs)?;
        Ok(())
    }
}

/// payload of styled text
/// |1-byte ended|4-byte n_spans|1-byte fg|1-byte bg|2-byte add_modifier|2-byte sub_modifier|4-byte strlen|n-byte str|...
fn decode_styled_text(src: &[u8]) -> Result<(Vec<Span<'static>>, bool)> {
    let mut cursor = Cursor::new(src);
    let ended = cursor.read_u8()?;
    let ended = ended != 0x00;
    let n_spans = cursor.read_u32::<LE>()?;
    let mut spans = Vec::with_capacity(n_spans as usize);
    for _ in 0..n_spans {
        let fg = cursor.read_u8()?;
        let fg = num_to_color(fg);
        let bg = cursor.read_u8()?;
        let bg = num_to_color(bg);
        let add_modifier = cursor.read_u16::<LE>()?;
        let add_modifier = Modifier::from_bits(add_modifier)
            .ok_or_else(|| Error::DecodeError("invalid add_modifier".to_owned()))?;
        let sub_modifier = cursor.read_u16::<LE>()?;
        let sub_modifier = Modifier::from_bits(sub_modifier)
            .ok_or_else(|| Error::DecodeError("invalid sub_modifier".to_owned()))?;
        let len = cursor.read_u32::<LE>()?;
        let mut content = vec![0u8; len as usize];
        cursor.read_exact(&mut content[..])?;
        spans.push(Span::styled(
            String::from_utf8(content)?,
            Style {
                fg,
                bg,
                add_modifier,
                sub_modifier,
            },
        ));
    }
    Ok((spans, ended))
}

fn encode_styled_text(spans: Vec<Span<'static>>, ended: bool) -> Result<Vec<u8>> {
    let mut bs = Vec::new();
    if ended {
        bs.write_all(&[0x01])?;
    } else {
        bs.write_all(&[0x00])?;
    }
    let n_spans = spans.len() as u32;
    bs.write_u32::<LE>(n_spans)?;
    for span in spans {
        let fg = color_to_num(span.style.fg);
        bs.write_u8(fg)?;
        let bg = color_to_num(span.style.bg);
        bs.write_u8(bg)?;
        let add_modifier = span.style.add_modifier.bits();
        bs.write_u16::<LE>(add_modifier)?;
        let sub_modifier = span.style.sub_modifier.bits();
        bs.write_u16::<LE>(sub_modifier)?;
        let len = span.content.len() as u32;
        bs.write_u32::<LE>(len)?;
        bs.write_all(span.content.as_bytes())?;
    }
    Ok(bs)
}

fn num_to_color(n: u8) -> Option<Color> {
    match n {
        0 => None,
        1 => Some(Color::Reset),
        30 => Some(Color::Black),
        31 => Some(Color::Red),
        32 => Some(Color::Green),
        33 => Some(Color::Yellow),
        34 => Some(Color::Blue),
        35 => Some(Color::Magenta),
        36 => Some(Color::Cyan),
        37 => Some(Color::Gray),
        90 => Some(Color::DarkGray),
        91 => Some(Color::LightRed),
        92 => Some(Color::LightGreen),
        93 => Some(Color::LightYellow),
        94 => Some(Color::LightBlue),
        95 => Some(Color::LightMagenta),
        96 => Some(Color::LightCyan),
        97 => Some(Color::White),
        _ => None,
    }
}

fn color_to_num(color: Option<Color>) -> u8 {
    match color {
        None => 0,
        Some(Color::Reset) => 1,
        Some(Color::Black) => 30,
        Some(Color::Red) => 31,
        Some(Color::Green) => 32,
        Some(Color::Yellow) => 33,
        Some(Color::Blue) => 34,
        Some(Color::Magenta) => 35,
        Some(Color::Cyan) => 36,
        Some(Color::Gray) => 37,
        Some(Color::DarkGray) => 90,
        Some(Color::LightRed) => 91,
        Some(Color::LightGreen) => 92,
        Some(Color::LightYellow) => 93,
        Some(Color::LightBlue) => 94,
        Some(Color::LightMagenta) => 95,
        Some(Color::LightCyan) => 96,
        Some(Color::White) => 97,
        Some(_) => 0,
    }
}

fn read_packet<R: Read>(mut reader: R, buf: &mut Vec<u8>) -> Result<usize> {
    let len = reader.read_u24::<LE>()?;
    let start = buf.len();
    buf.reserve(len as usize);
    for _ in 0..len {
        buf.push(0u8);
    }
    reader.read_exact(&mut buf[start..])?;
    Ok(len as usize)
}

fn write_packet<W: Write>(mut writer: W, buf: &[u8]) -> Result<()> {
    debug_assert!(buf.len() <= 0xff_ffff);
    writer.write_u24::<LE>(buf.len() as u32)?;
    writer.write_all(&buf[..])?;
    Ok(())
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_read_packet() {
        let input = vec![1u8, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4];
        let pkt = Packet::read_from(&mut &input[..]).unwrap();
        println!("pkt={:?}", pkt);
    }
}
