use crate::style::line::ArcSpan;
use tui::style::Style;
use ansi_parser::{AnsiParser, AnsiSequence, Output, AnsiParseIterator};

/// 将ansi字符流转换为ArcSpan流
pub struct AnsiBridge {
    style: Style,
    // spaces_per_tab: usize,
    reserve_cr: bool,
    // if this field is set to true, it will fill up additional spaces
    // if the char's width is less than width_cjk.
    // it is helpful for fonts that does not display correctly in cjk environment
    pad_non_cjk: bool,
    // buffer string to handle incomplete ansi sequence
    buf: String,
}

impl AnsiBridge {

    pub fn fill(&mut self, input: String) {
        if self.buf.is_empty() {
            std::mem::replace(&mut self.buf, input);
        }
        todo!()
    }

    pub fn next_span(&mut self) -> Option<ArcSpan> {
        todo!()
    }

    // pub fn parse(&mut self, input: &str) -> Vec<ArcSpan> {
    //     for output in input.ansi_parse() {
    //         match output {
    //             Output::TextBlock(s) => {
    //                 // let mut line = Vec::new();
    //                 let mut line = StyledLine::empty();
    //                 let mut adapter = CJKStringAdapter::new();
    //                 let mut newline_end = false;
    //                 for c in s.chars() {
    //                     newline_end = false;
    //                     match c {
    //                         '\n' => {
    //                             line.extract(&mut adapter, self.style, true);
    //                             Self::push_line(&mut lines, line);
    //                             line = StyledLine::empty();
    //                             newline_end = true;
    //                         }
    //                         '\t' => {
    //                             // tui does not handle tab correctly, so convert to spaces
    //                             let width = adapter.width();
    //                             let tabs = width / self.spaces_per_tab;
    //                             let num = (tabs + 1) * self.spaces_per_tab - width;
    //                             for _ in 0..num {
    //                                 adapter.push(' ');
    //                             }
    //                         }
    //                         '\r' if !self.reserve_cr => (),
    //                         _ => {
    //                             adapter.push(c);
    //                             if self.pad_non_cjk {
    //                                 if let (Some(w), Some(cw)) = (c.width(), c.width_cjk()) {
    //                                     if w < cw {
    //                                         for _ in w..cw {
    //                                             adapter.push_additional(' ');
    //                                         }
    //                                     }
    //                                 }
    //                             }
    //                         }
    //                     }
    //                 }
    //                 if !newline_end {
    //                     line.extract(&mut adapter, self.style, false);
    //                     Self::push_line(&mut lines, line);
    //                 }
    //             }
    //             Output::Escape(seq) => match seq {
    //                 AnsiSequence::SetGraphicsMode(sgm) => match sgm.len() {
    //                     0 => {
    //                         self.style = Style::default();
    //                     }
    //                     _ => {
    //                         for code in sgm {
    //                             self.style = super::apply_ansi_sgr(self.style, code);
    //                         }
    //                     }
    //                 },
    //                 _ => {
    //                     // eprintln!("unexpected ansi escape {:?}", seq)
    //                 }
    //             },
    //         }
    //     }

    //     todo!()
    // }
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
    use ansi_parser::AnsiParser;
    #[test]
    fn test_ansi_parse() {
        let input = String::from_utf8(b"\x1b[0mabc".to_vec()).unwrap();
        let out: Vec<_> = input.ansi_parse().collect();
        assert_eq!(3, out.len());

    }
}