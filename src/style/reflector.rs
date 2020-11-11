use tui::style::Style;
use std::collections::VecDeque;
use crate::style::StyledLine;
use ansi_parser::{AnsiParser, AnsiSequence, Output};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// convert ansi sequence and text blocks to tui text
pub struct Reflector {
    style: Style,
    spaces_per_tab: usize,
    reserve_cr: bool,
    /// if this field is set to true, it will fill up additional spaces
    /// if the char's width is less than width_cjk.
    /// it is helpful for fonts that does not display correctly in cjk environment
    pad_non_cjk: bool,
}

impl Default for Reflector {
    fn default() -> Self {
        Self {
            style: Style::default(),
            spaces_per_tab: 8,
            reserve_cr: false,
            pad_non_cjk: false,
        }
    }
}

impl Reflector {

    pub fn spaces_per_tab(mut self, spaces_per_tab: usize) -> Self {
        self.spaces_per_tab = spaces_per_tab;
        self
    }

    pub fn reserve_cr(mut self, reserve_cr: bool) -> Self {
        self.reserve_cr = reserve_cr;
        self
    }

    pub fn pad_non_cjk(mut self, pad_non_cjk: bool) -> Self {
        self.pad_non_cjk = pad_non_cjk;
        self
    }
}

impl Reflector {
    /// accept input string and convert to tui spans
    pub fn reflect(&mut self, input: impl AsRef<str>) -> VecDeque<StyledLine> {
        let mut lines = VecDeque::new();
        for output in input.as_ref().ansi_parse() {
            match output {
                Output::TextBlock(s) => {
                    // let mut line = Vec::new();
                    let mut line = StyledLine::empty();
                    let mut adapter = CJKStringAdapter::new();
                    let mut newline_end = false;
                    for c in s.chars() {
                        newline_end = false;
                        match c {
                            '\n' => {
                                line.extract(&mut adapter, self.style, true);
                                Self::push_line(&mut lines, line);
                                line = StyledLine::empty();
                                newline_end = true;
                            }
                            '\t' => {
                                // tui does not handle tab correctly, so convert to spaces
                                let width = adapter.width();
                                let tabs = width / self.spaces_per_tab;
                                let num = (tabs + 1) * self.spaces_per_tab - width;
                                for _ in 0..num {
                                    adapter.push(' ');
                                }
                            }
                            '\r' if !self.reserve_cr => (),
                            _ => {
                                adapter.push(c);
                                if self.pad_non_cjk {
                                    if let (Some(w), Some(cw)) = (c.width(), c.width_cjk()) {
                                        if w < cw {
                                            for _ in w..cw {
                                                adapter.push_additional(' ');
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if !newline_end {
                        line.extract(&mut adapter, self.style, false);
                        Self::push_line(&mut lines, line);
                    }
                }
                Output::Escape(seq) => match seq {
                    AnsiSequence::SetGraphicsMode(sgm) => match sgm.len() {
                        0 => {
                            self.style = Style::default();
                        }
                        _ => {
                            for code in sgm {
                                self.style = super::apply_ansi_sgr(self.style, code);
                            }
                        }
                    },
                    _ => {
                        // eprintln!("unexpected ansi escape {:?}", seq)
                    }
                },
            }
        }
        lines
    }

    fn try_concat_last_line(lines: &mut VecDeque<StyledLine>, line: &mut StyledLine) -> bool {
        if let Some(last_line) = lines.back_mut() {
            if !last_line.ended {
                last_line
                    .spans
                    .extend(std::mem::replace(&mut line.spans, vec![]));
                last_line.orig.push_str(&line.orig);
                last_line.ended = line.ended;
                return true;
            }
        }
        false
    }

    fn push_line(lines: &mut VecDeque<StyledLine>, mut line: StyledLine) {
        if !Self::try_concat_last_line(lines, &mut line) {
            lines.push_back(line);
        }
    }
}


#[derive(Debug, Clone)]
pub struct CJKStringAdapter {
    os: String,
    es: String,
    width: usize,
}

impl CJKStringAdapter {
    pub fn new() -> Self {
        Self {
            os: String::new(),
            es: String::new(),
            width: 0,
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn drain_origin_and_extended(&mut self) -> (String, String) {
        let os = std::mem::replace(&mut self.os, String::new());
        let es = std::mem::replace(&mut self.es, String::new());
        self.width = 0;
        (os, es)
    }

    pub fn push(&mut self, c: char) {
        self.os.push(c);
        self.es.push(c);
        self.width += c.width_cjk().unwrap_or(0);
    }

    pub fn push_additional(&mut self, c: char) {
        self.es.push(c);
        self.width += c.width_cjk().unwrap_or(0);
    }

    pub fn push_str(&mut self, s: &str) {
        self.os.push_str(s);
        self.es.push_str(s);
        self.width += s.width_cjk();
    }

    pub fn push_additional_str(&mut self, s: &str) {
        self.es.push_str(s);
        self.width += s.width_cjk();
    }

    pub fn into_origin_and_extended(self) -> (String, String) {
        (self.os, self.es)
    }
}
