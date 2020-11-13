use crate::ui::span::ArcSpan;
use unicode_width::UnicodeWidthChar;
use std::sync::Arc;
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq)]
pub enum RawLine {
    Owned(String),
    Ref(Arc<str>, usize, usize),
}

impl RawLine {

    pub fn owned(line: String) -> Self {
        Self::Owned(line)
    }

    pub fn borrowed(line: Arc<str>, start: usize, end: usize) -> Self {
        debug_assert!(end >= start);
        Self::Ref(line, start, end)
    }

    pub fn err(line: impl AsRef<str>) -> Self {
        let formatted = format!("{}{}{}\r\n", termion::style::Invert, line.as_ref(), termion::style::Reset);
        Self::Owned(formatted)
    }

    pub fn note(line: impl AsRef<str>) -> Self {
        let formatted = format!("{}{}{}\r\n", termion::color::Fg(termion::color::LightBlue), line.as_ref(), termion::style::Reset);
        Self::Owned(formatted)
    }

    pub fn raw(line: impl AsRef<str>) -> Self {
        let formatted = format!("{}{}\r\n", termion::style::Reset, line.as_ref());
        Self::Owned(formatted)
    }

    pub fn ended(&self) -> bool {
        match self {
            Self::Owned(s) => s.ends_with('\n'),
            Self::Ref(s, start, end) => s.as_ref()[*start..*end].ends_with('\n'),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Owned(s) => s.len(),
            Self::Ref(_, start, end) => *end - *start,
        }
    }

    pub fn push_line(&mut self, line: impl AsRef<str>) {
        let mut s = match self {
            Self::Owned(s) => {
                s.push_str(line.as_ref());
                return;
            },
            Self::Ref(s, start, end) => s.as_ref()[*start..*end].to_owned(),
        };
        s.push_str(line.as_ref());
        *self = Self::Owned(s);
    }
}

#[derive(Debug, Clone)]
pub struct RawLineBuffer {
    lines: VecDeque<RawLine>,
    capacity: usize,
}

impl RawLineBuffer {

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            lines: VecDeque::new(),
            capacity,
        }
    }

    pub fn unbounded() -> Self {
        Self::with_capacity(0)
    }

    pub fn push_line(&mut self, line: RawLine) {
        if let Some(last_line) = self.lines.back_mut() {
            if !last_line.ended() {
                last_line.push_line(line);
                return;
            }
        }
        // empty lines
        self.lines.push_back(line);
        while self.capacity > 0 && self.capacity > self.lines.len() {
            self.lines.pop_front();
        }
    }

    pub fn into_inner(self) -> VecDeque<RawLine> {
        self.lines
    }

    pub fn to_vec(&self) -> Vec<RawLine> {
        self.lines.iter().cloned().collect()
    }

    pub fn into_vec(self) -> Vec<RawLine> {
        self.lines.into_iter().collect()
    }
}

impl AsRef<str> for RawLine {
    fn as_ref(&self) -> &str {
        match self {
            Self::Owned(s) => s,
            Self::Ref(s, start, end) => &s.as_ref()[*start..*end],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub spans: Vec<ArcSpan>,
}

impl Line {
    pub fn new(spans: Vec<ArcSpan>) -> Self {
        Self{spans}
    }

    pub fn ended(&self) -> bool {
        self.spans.last().map(|s| s.ended).unwrap_or(false)
    }

    pub fn wrap(&self, max_width: usize, cjk: bool) -> WrapLine {
        let mut lines= vec![];
        wrap_line(&self, max_width, cjk, &mut lines);
        WrapLine(lines)
    }

    pub fn note(content: impl Into<String>) -> Self {
        Self{spans: vec![ArcSpan::note(content)]}
    }

    pub fn err(content: impl Into<String>) -> Self {
        Self{spans: vec![ArcSpan::err(content)]}
    }

    pub fn raw(content: impl Into<String>) -> Self {
        Self{spans: vec![ArcSpan::raw(content)]}
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WrapLine(pub Vec<Line>);

impl WrapLine {

    pub fn wrap(&self, max_width: usize, cjk: bool) -> Self {
        let mut lines = vec![];
        for line in &self.0 {
            wrap_line(line, max_width, cjk, &mut lines);
        }
        WrapLine(lines)
    }
}

/// 根据指定行宽将单行拆解为多行，并添加到可变数组中
pub fn wrap_line(line: &Line, max_width: usize, cjk: bool, lines: &mut Vec<Line>) {
    // let mut lines = Vec::new();
    let mut curr_line = if lines.last().map(|l| !l.ended()).unwrap_or(false) {
        lines.pop().unwrap().spans
    } else {
        Vec::new()
    };
    let mut curr_width = curr_line.iter()
        .map(|span| span.width(cjk))
        .sum();
    
    for span in &line.spans {
        // 判断宽度是否超过限制
        if span.width(cjk) + curr_width <= max_width {
            // 合并到当前行
            if append_span(&mut curr_line, span.clone()) {
                // 行结束
                lines.push(Line::new(curr_line.drain(..).collect()));
                curr_width = 0;
            } else {
                curr_width += span.width(cjk);
            }
        } else {
            let new_style = span.style;
            let new_ended = span.ended;
            let mut new_content = String::new();
            for c in span.content().chars() {
                let cw = if cjk { c.width_cjk() } else { c.width() }.unwrap_or(0);
                if curr_width + cw <= max_width {
                    new_content.push(c);
                    curr_width += cw;
                } else {
                    // exceeds max width
                    // current char must be wrap to next line, so this span is partial
                    let new_span = ArcSpan::new(
                        std::mem::replace(&mut new_content, String::new()),
                        new_style,
                        false,
                    );
                    append_span(&mut curr_line, new_span);
                    lines.push(Line::new(std::mem::replace(&mut curr_line, Vec::new())));
                    // we need to push current char to new content
                    new_content.push(c);
                    curr_width = cw;
                }
            }
            // concat last span to curr_line
            if !new_content.is_empty() {
                let new_span = ArcSpan::new(new_content, new_style, new_ended);
                curr_line.push(new_span);
            }
        }
    }
    if !curr_line.is_empty() {
        lines.push(Line::new(curr_line));
    }
} 

// 将span合并进行，返回行是否结束
fn append_span(line: &mut Vec<ArcSpan>, span: ArcSpan) -> bool {
    let ended = span.ended;
    if let Some(last_span) = line.last_mut() {
        if last_span.style == span.style {
            // 仅当格式相同时合并
            last_span.push_str(span.content());
            last_span.ended = ended;
            return ended;
        }
    }
    line.push(span);
    ended
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::ui::style::{Style, Color};

    #[test]
    fn test_wrap_single_line() {
        let line = Line::new(vec![ended_span("helloworld")]);
        let wl = line.wrap(20, true);
        assert_eq!(wl, WrapLine(vec![
            Line::new(vec![ended_span("helloworld")]),
        ]));

        let line = Line::new(vec![ended_span("helloworld")]);
        let wl = line.wrap(4, true);
        assert_eq!(wl, WrapLine(vec![
            Line::new(vec![partial_span("hell")]), 
            Line::new(vec![partial_span("owor")]), 
            Line::new(vec![ended_span("ld")]),
        ]));

        let line = Line::new(vec![ended_span("中国人")]);
        let wl = line.wrap(3, true);
        assert_eq!(wl, WrapLine(vec![
            Line::new(vec![partial_span("中")]),
            Line::new(vec![partial_span("国")]),
            Line::new(vec![ended_span("人")]),
        ]))
    }

    #[test]
    fn test_wrap_multi_line() {
        let line = Line::new(vec![ended_span("helloworld")]);
        let wl = line.wrap(4, true);
        let wl = wl.wrap(6, true);
        assert_eq!(wl, WrapLine(vec![
            Line::new(vec![partial_span("hellow")]),
            Line::new(vec![ended_span("orld")]),
        ]));
    }

    #[test]
    fn test_warp_multi_style() {
        let line = Line::new(vec![red_span("hello"), ended_span("world")]);
        let wl = line.wrap(4, true);
        assert_eq!(wl, WrapLine(vec![
            Line::new(vec![red_span("hell")]),
            Line::new(vec![red_span("o"), partial_span("wor")]),
            Line::new(vec![ended_span("ld")]),
        ]));
    }

    fn ended_span(s: &str) -> ArcSpan {
        ArcSpan::new(s, Style::default(), true)
    }

    fn partial_span(s: &str) -> ArcSpan {
        ArcSpan::new(s, Style::default(), false)
    }

    fn red_span(s: &str) -> ArcSpan {
        ArcSpan::new(s, Style::default().fg(Color::Red), false)
    }
}
