use crate::ui::span::Span;
use crate::ui::style::{Color, Modifier, Style};
use std::sync::Arc;

pub mod clear {

    #[derive(Debug, Clone)]
    pub struct ClearCells(pub u16);

    impl std::fmt::Display for ClearCells {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "\x1b[{}X", self.0)
        }
    }
}

/// 将ansi字符流转换为ArcSpan流
#[derive(Debug)]
pub struct AnsiParser {
    style: Style,
    // spaces_per_tab: usize,
    reserve_cr: bool,
    // buffer string to handle incomplete ansi sequence
    buf: Option<Arc<str>>,
    next: usize,
}

impl AnsiParser {
    pub fn new() -> Self {
        Self {
            style: Style::default(),
            reserve_cr: false,
            buf: None,
            next: 0,
        }
    }

    pub fn reserve_cr(mut self, reserve_cr: bool) -> Self {
        self.reserve_cr = reserve_cr;
        self
    }

    pub fn fill(&mut self, input: impl Into<String>) {
        match self.buf.take() {
            Some(buf) => {
                let mut s = buf.as_ref().to_owned();
                s.push_str(&input.into());
                self.buf = Some(Arc::from(s));
            }
            None => self.buf = Some(Arc::from(input.into())),
        }
    }

    pub fn next_span(&mut self) -> Option<Span> {
        loop {
            match self.next_snippet(self.next) {
                Snippet::End => {
                    self.buf.take();
                    self.next = 0;
                    return None;
                }
                Snippet::Incomplete => {
                    return None;
                }
                Snippet::Style(style, next) => {
                    self.next = next;
                    self.style = style;
                }
                Snippet::Span(span, next) => {
                    self.next = next;
                    return Some(span);
                }
            }
        }
    }

    fn next_snippet(&self, start: usize) -> Snippet {
        if self.buf.is_none() {
            return Snippet::End;
        }
        let buf = self.buf.as_ref().unwrap();
        if start == buf.as_ref().len() {
            return Snippet::End;
        }
        if buf.as_ref()[start..].starts_with("\x1b[") {
            let sgm_start = start + 2;
            match self.parse_sgm(buf, sgm_start) {
                Some((style, next)) => {
                    return Snippet::Style(style, next);
                }
                None => {
                    log::warn!("unrecognized ansi escape: {:?}", &buf.as_ref()[sgm_start..]);
                    return Snippet::Incomplete;
                }
            }
        }
        match self.parse_text(self.next) {
            Some((span, next)) => Snippet::Span(span, next),
            None => Snippet::End,
        }
    }

    fn parse_text(&self, start: usize) -> Option<(Span, usize)> {
        // probably parse the text
        let buf = self.buf.as_ref().cloned().unwrap();
        match buf.as_ref()[start..].find(|c| c == '\x1b' || c == '\n') {
            None => {
                // 整体都是文本，且无断行
                let buflen = buf.as_ref().len();
                let span = Span::new(&buf[start..buflen], self.style);
                Some((span, buflen))
            }
            Some(pos) => {
                let pos = start + pos;
                let c = buf.as_ref().as_bytes()[pos];
                if c == b'\x1b' {
                    let span = Span::new(&buf[start..pos], self.style);
                    Some((span, pos))
                } else {
                    // 存在行结束符，包含结束符
                    let span = Span::new(&buf[start..pos + 1], self.style);
                    Some((span, pos + 1))
                }
            }
        }
    }

    fn parse_sgm(&self, buf: &Arc<str>, start: usize) -> Option<(Style, usize)> {
        if start == buf.as_ref().len() {
            // 不完整
            return None;
        }

        buf.as_ref()[start..]
            .find(|c| c != ';' && (c < '0' || c > '9'))
            .map(|pos| {
                let pos = start + pos;
                match buf.as_ref().as_bytes()[pos] {
                    b'm' => {
                        let mut style = self.style;
                        let mut n = 0;
                        for c in buf.as_ref()[start..pos].chars() {
                            match c {
                                ';' => {
                                    style = apply_ansi_sgr(style, n);
                                    n = 0;
                                }
                                '0'..='9' => {
                                    n *= 10;
                                    n += (c as u8) - b'0';
                                }
                                other => {
                                    unreachable!("unreachable char '{}' in sgm sequence", other)
                                }
                            }
                        }
                        style = apply_ansi_sgr(style, n);
                        (style, pos + 1)
                    }
                    b'z' => {
                        log::warn!("ignore escape sequence 'ESC[Nz'");
                        (self.style, pos + 1)
                    }
                    _ => {
                        log::error!(
                            "unsupported CSI sequence {:?}",
                            buf.as_ref()[start..=pos].as_bytes()
                        );
                        (self.style, pos + 1)
                    }
                }
            })
    }
}

enum Snippet {
    Style(Style, usize),
    Span(Span, usize),
    Incomplete,
    End,
}

fn apply_ansi_sgr(mut style: Style, code: u8) -> Style {
    match code {
        0 => Style::default(),
        1 => style.add_modifier(Modifier::BOLD),
        2 => style.add_modifier(Modifier::DIM),
        3 => style.add_modifier(Modifier::ITALIC),
        4 => style.add_modifier(Modifier::UNDERLINED),
        5 => style.add_modifier(Modifier::SLOW_BLINK),
        6 => style.add_modifier(Modifier::RAPID_BLINK),
        7 => style.add_modifier(Modifier::REVERSED),
        8 => style.add_modifier(Modifier::HIDDEN),
        9 => style.add_modifier(Modifier::CROSSED_OUT),
        21 => style.remove_modifier(Modifier::BOLD),
        22 => style.remove_modifier(Modifier::DIM),
        23 => style.remove_modifier(Modifier::ITALIC),
        24 => style.remove_modifier(Modifier::UNDERLINED),
        25 => style
            .remove_modifier(Modifier::SLOW_BLINK)
            .remove_modifier(Modifier::RAPID_BLINK),
        27 => style.remove_modifier(Modifier::REVERSED),
        28 => style.remove_modifier(Modifier::HIDDEN),
        29 => style.remove_modifier(Modifier::CROSSED_OUT),
        // frontend color
        30 => style.fg(Color::Black),
        31 => style.fg(Color::Red),
        32 => style.fg(Color::Green),
        33 => style.fg(Color::Yellow),
        34 => style.fg(Color::Blue),
        35 => style.fg(Color::Magenta),
        36 => style.fg(Color::Cyan),
        37 => style.fg(Color::Gray),
        38 => unimplemented!(),
        39 => {
            style.fg = None;
            style
        }
        90 => style.fg(Color::DarkGray),
        91 => style.fg(Color::LightRed),
        92 => style.fg(Color::LightGreen),
        93 => style.fg(Color::LightYellow),
        94 => style.fg(Color::LightBlue),
        95 => style.fg(Color::LightMagenta),
        96 => style.fg(Color::LightCyan),
        97 => style.fg(Color::White),
        // backend color
        40 => style.bg(Color::Black),
        41 => style.bg(Color::Red),
        42 => style.bg(Color::Green),
        43 => style.bg(Color::Yellow),
        44 => style.bg(Color::Blue),
        45 => style.bg(Color::Magenta),
        46 => style.bg(Color::Cyan),
        47 => style.bg(Color::Gray),
        48 => unimplemented!(),
        49 => {
            style.bg = None;
            style
        }
        100 => style.bg(Color::DarkGray),
        101 => style.bg(Color::LightRed),
        102 => style.bg(Color::LightGreen),
        103 => style.bg(Color::LightYellow),
        104 => style.bg(Color::LightBlue),
        105 => style.bg(Color::LightMagenta),
        106 => style.bg(Color::LightCyan),
        107 => style.bg(Color::White),
        _ => panic!("unknown SGR argument {}", code),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ansi_span_stream() {
        let mut ss = AnsiParser::new();
        ss.fill("hello");
        assert_eq!(Some(Span::new("hello", Style::default())), ss.next_span());
        assert_eq!(None, ss.next_span());
        assert!(ss.buf.is_none());
        ss.fill("hello\nworld");
        assert_eq!(Some(Span::new("hello\n", Style::default())), ss.next_span());
        assert_eq!(Some(Span::new("world", Style::default())), ss.next_span());
        assert_eq!(None, ss.next_span());
        ss.fill("\x1b[37m");
        assert_eq!(None, ss.next_span());
        assert_eq!(Style::default().fg(Color::Gray), ss.style);
        ss.fill("hello");
        assert_eq!(
            Some(Span::new("hello", Style::default().fg(Color::Gray),)),
            ss.next_span()
        );
        ss.fill("\x1b[mworld\n");
        assert_eq!(Some(Span::new("world\n", Style::default())), ss.next_span());
    }
    use std::fs::File;
    use std::io::Read;
    #[test]
    fn test_ansi_server_log() {
        let mut s = String::new();
        let mut f = File::open("server.log").unwrap();
        f.read_to_string(&mut s).unwrap();
        let mut ss = AnsiParser::new();
        ss.fill(s);
        while let Some(span) = ss.next_span() {
            println!("span={:?}", span);
        }
    }
}
