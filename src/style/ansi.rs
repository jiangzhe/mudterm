use crate::style::line::ArcSpan;
use tui::style::Style;
use ansi_parser::{AnsiParser, AnsiSequence, Output};
use ansi_parser::parse_escape;

#[derive(Debug)]
/// 将ansi字符流转换为ArcSpan流
pub struct AnsiAdapter {
    style: Style,
    // spaces_per_tab: usize,
    reserve_cr: bool,
    // buffer string to handle incomplete ansi sequence
    buf: String,
    next: usize,
}

impl AnsiAdapter {

    pub fn new() -> Self {
        Self{
            style: Style::default(),
            reserve_cr: false,
            buf: String::new(),
            next: 0,
        }
    }

    pub fn reserve_cr(mut self, reserve_cr: bool) -> Self {
        self.reserve_cr = reserve_cr;
        self
    }

    pub fn fill(&mut self, input: impl Into<String>) {
        if self.buf.is_empty() {
            self.buf = input.into();
            return;
        }
        self.buf.push_str(&input.into());
    }

    fn parse_escape(&mut self, start: usize, end: usize, ended: bool) -> Option<ArcSpan> {
        for output in self.buf[start..end].ansi_parse() {
            match output {
                Output::Escape(AnsiSequence::SetGraphicsMode(sgm)) => {
                    match sgm.len() {
                        0 => self.style = Style::default(),
                        _ => {
                            for code in sgm {
                                self.style = super::apply_ansi_sgr(self.style, code);
                            }
                        }
                    }
                }
                Output::Escape(esc) => eprintln!("unexpected ansi escape {:?}", esc),
                Output::TextBlock(s) => {
                    let content = if !self.reserve_cr && s.len() > 0 && s.as_bytes()[s.len()-1] == b'\r' {
                        s[..s.len()-1].to_owned()
                    } else {
                        s.to_owned()
                    };
                    return Some(ArcSpan::new(content, self.style, ended));
                }
            }
        }
        // 仅存在Escape而不存在Text
        None
    }

    fn apply_sgm(&mut self, sgm: Vec<u8, _>) {
        match sgm.len() {
            0 => self.style = Style::default(),
            _ => {
                for code in sgm {
                    self.style = super::apply_ansi_sgr(self.style, code);
                }
            }
        }
    }

    fn parse_escape_to_end(&mut self, start: usize) -> Option<ArcSpan> {
        let mut escape = false;
        for output in self.buf[start..].ansi_parse() {
            match output {
                Output::Escape(AnsiSequence::SetGraphicsMode(sgm)) => {
                    self.apply_sgm(sgm);
                    escape = true;
                }
                Output::Escape(esc) => eprintln!("unexpected ansi escape {:?}", esc),
                Output::TextBlock(s) => {
                    if !escape {
                        eprintln!("escape sequence not parsed, s={}", s);
                        // 该文本不可作为输出
                        // 用户应将后续数据输入后再调用next_span方法
                        self.buf = s.to_owned();
                        self.next = 0;
                        return None;
                    }
                    let content = if !self.reserve_cr && s.len() > 0 && s.as_bytes()[s.len()-1] == b'\r' {
                        s[..s.len()-1].to_owned()
                    } else {
                        s.to_owned()
                    };
                    self.buf.clear();
                    self.next = 0;
                    return Some(ArcSpan::new(content, self.style, false));
                }
            }
        }
        None
    }

    fn parse_text_to_end(&mut self, start: usize) -> Option<ArcSpan> {
        // all text
        let content = if !self.reserve_cr && self.buf.len() > 0 && self.buf.as_bytes()[self.buf.len()-1] == b'\r' {
            self.buf[start..self.buf.len()-1].to_owned()
        } else {
            self.buf[start..].to_owned()
        };
        self.buf.clear();
        self.next = 0;
        Some(ArcSpan::new(content, self.style, false))
    }

    fn parse_escape_at_beginnig(&mut self) {
        // todo
        todo!()
    }

    pub fn next_span(&mut self) -> Option<ArcSpan> {
        // 由于每次均将next置为下一位，无可处理字符时清空缓存并返回空
        if self.next == self.buf.len() {
            self.buf.clear();
            self.next = 0;
            return None;
        }
        let start = if self.next == 0 && self.buf.as_bytes()[self.next] == b'\x1d' {
            
        } else if self.next > 0 && self.buf.as_bytes()[self.next-1] == b'\x1b' {
            self.next - 1
        } else {
            self.next
        };
        if let Some(pos) = self.buf[self.next..].find(|c| c == '\u{1d}' || c == '\n') {
            let pos = self.next + pos;
            let ended = self.buf.as_bytes()[pos] == b'\n';
            if let Some(span) = self.parse_escape(start, pos, ended) {
                self.next = pos + 1;
                return Some(span);
            } 
            self.next = pos + 1;
            return self.next_span();
        } 
        if self.next != start {
            // 当首字母ESC时，且其后不包含ESC或\n，则剩余应为一个SGR+一个断句
            return self.parse_escape_to_end(start);
        } 
        self.parse_text_to_end(start)
    }
}

#[cfg(test)]
mod tests {
    use ansi_parser::AnsiParser;
    use super::*;

    #[test]
    fn test_ansi_parse() {
        let input = String::from_utf8(b"\x1b[0mabc".to_vec()).unwrap();
        let out: Vec<_> = input.ansi_parse().collect();
        assert_eq!(3, out.len());
    }

    #[test]
    fn test_ansi_adapter() {
        let mut adapter = AnsiAdapter::new();
        adapter.fill("hello");
        assert_eq!(Some(ArcSpan::new("hello", Style::default(), false)), adapter.next_span());
        assert_eq!(None, adapter.next_span());
        assert!(adapter.buf.is_empty());
        adapter.fill("hello\nworld");
        assert_eq!(Some(ArcSpan::new("hello", Style::default(), true)), adapter.next_span());
        assert_eq!(Some(ArcSpan::new("world", Style::default(), false)), adapter.next_span());
        assert_eq!(None, adapter.next_span());
        adapter.fill("\x1d[37m");
        assert_eq!(None, adapter.next_span());
    }
}