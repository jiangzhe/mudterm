use crate::ui::span::Span;
use crate::ui::width::AppendWidthTab8;
use std::collections::VecDeque;
use std::sync::Arc;

#[derive(Debug)]
pub struct RawLine(Arc<str>, usize, usize);

impl PartialEq for RawLine {
    fn eq(&self, other: &Self) -> bool {
        self.content() == other.content()
    }
}

impl Clone for RawLine {
    fn clone(&self) -> Self {
        let Self(s, start, end) = self;
        // 判断如果是整行，或者长度不小于128，直接复制
        if (*start == 0 && *end == s.len()) || *end >= *start + 128 {
            return Self(s.clone(), *start, *end);
        }
        // 较短时复制字符串
        let s = s.as_ref()[*start..*end].to_owned();
        Self::owned(s)
    }
}

impl AsRef<str> for RawLine {
    fn as_ref(&self) -> &str {
        self.content()
    }
}

impl RawLine {
    pub fn owned(line: String) -> Self {
        let len = line.len();
        Self(Arc::from(line), 0, len)
    }

    pub fn borrowed(line: Arc<str>, start: usize, end: usize) -> Self {
        debug_assert!(end >= start);
        Self(line, start, end)
    }

    pub fn err(line: impl AsRef<str>) -> Self {
        let formatted = format!(
            "{}{}{}\r\n",
            termion::style::Invert,
            line.as_ref(),
            termion::style::Reset
        );
        Self::owned(formatted)
    }

    pub fn note(line: impl AsRef<str>) -> Self {
        let formatted = format!(
            "{}{}{}\r\n",
            termion::color::Fg(termion::color::LightBlue),
            line.as_ref(),
            termion::style::Reset
        );
        Self::owned(formatted)
    }

    pub fn raw(line: impl AsRef<str>) -> Self {
        let formatted = format!("{}{}\r\n", termion::style::Reset, line.as_ref());
        Self::owned(formatted)
    }

    pub fn ended(&self) -> bool {
        let Self(s, start, end) = self;
        s.as_ref()[*start..*end].ends_with('\n')
    }

    pub fn len(&self) -> usize {
        let Self(_, start, end) = self;
        *end - *start
    }

    pub fn content(&self) -> &str {
        let Self(s, start, end) = self;
        &s.as_ref()[*start..*end]
    }

    // maybe expensive because we need to copy the referenced string
    pub fn push_line(&mut self, line: impl AsRef<str>) -> bool {
        if self.ended() {
            // already ended, do not append
            return false;
        }
        let Self(s, start, end) = self;
        let mut s = s.as_ref()[*start..*end].to_owned();
        s.push_str(line.as_ref());
        *self = Self::owned(s);
        true
    }
}

#[derive(Debug, Clone)]
pub struct RawLines {
    lines: VecDeque<RawLine>,
    capacity: usize,
}

impl RawLines {
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

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub spans: Vec<Span>,
}

impl Line {
    pub fn new(spans: Vec<Span>) -> Self {
        Self { spans }
    }

    pub fn single(span: Span) -> Self {
        Self { spans: vec![span] }
    }

    pub fn ended(&self) -> bool {
        self.spans
            .last()
            .map(|s| s.content.ends_with('\n'))
            .unwrap_or(false)
    }

    pub fn display_width(&self, cjk: bool) -> usize {
        self.append_width(0, cjk)
    }

    pub fn wrap(&self, max_width: usize, cjk: bool) -> WrapLine {
        let mut lines = vec![];
        wrap_line(&self, max_width, cjk, &mut lines);
        WrapLine(lines)
    }

    pub fn fmt_note(content: impl Into<String>) -> Self {
        Self {
            spans: vec![Span::fmt_note(content)],
        }
    }

    pub fn fmt_err(content: impl Into<String>) -> Self {
        Self {
            spans: vec![Span::fmt_err(content)],
        }
    }

    pub fn fmt_raw(content: impl Into<String>) -> Self {
        Self {
            spans: vec![Span::fmt_raw(content)],
        }
    }

    pub fn push_span(&mut self, span: Span) -> bool {
        if self.ended() {
            return false;
        }
        self.spans.push(span);
        true
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WrapLine(pub Vec<Line>);

impl WrapLine {
    pub fn reshape(&self, max_width: usize, cjk: bool) -> Self {
        let mut lines = vec![];
        for line in &self.0 {
            wrap_line(line, max_width, cjk, &mut lines);
        }
        WrapLine(lines)
    }

    pub fn ended(&self) -> bool {
        self.0.last().map(|l| l.ended()).unwrap_or(false)
    }

    /// when calling this method, the max_width should be identical to previous setting
    /// otherwise, please call reshape() method at last
    pub fn push_span(&mut self, span: Span, max_width: usize, cjk: bool) -> bool {
        if self.ended() {
            return false;
        }
        if self.0.is_empty() {
            self.0.push(Line::new(vec![span]));
            return true;
        }
        let last_line = self.0.last_mut().unwrap();
        last_line.push_span(span);
        if last_line.display_width(cjk) > max_width {
            // exceeds max width, must wrap
            let wl = last_line.wrap(max_width, cjk);
            self.0.pop();
            self.0.extend(wl.0);
        }
        true
    }
}

/// 根据指定行宽将单行拆解为多行，并添加到可变数组中
pub fn wrap_line(line: &Line, max_width: usize, cjk: bool, lines: &mut Vec<Line>) {
    let mut curr_line = if lines.last().map(|l| !l.ended()).unwrap_or(false) {
        lines.pop().unwrap().spans
    } else {
        Vec::new()
    };
    let mut curr_width = curr_line.append_width(0, cjk);
    for span in &line.spans {
        // 判断宽度是否超过限制
        let next_width = span.append_width(curr_width, cjk);
        if next_width <= max_width {
            // 合并到当前行
            append_span(&mut curr_line, span.clone());
            if curr_line.last().unwrap().ended() {
                // 行结束
                lines.push(Line::new(curr_line.drain(..).collect()));
                curr_width = 0;
            } else {
                curr_width = next_width;
            }
        } else {
            let new_style = span.style;
            // let new_ended = span.ended;
            let mut new_content = String::new();
            for c in span.content.chars() {
                // let cw = if cjk { c.width_cjk() } else { c.width() }.unwrap_or(0);
                let next_width = c.append_width(curr_width, cjk);
                if next_width <= max_width {
                    new_content.push(c);
                    curr_width = next_width;
                } else {
                    // exceeds max width
                    // current char must be wrap to next line, so this span is partial
                    let new_span = Span::new(
                        std::mem::replace(&mut new_content, String::new()),
                        new_style,
                    );
                    append_span(&mut curr_line, new_span);
                    lines.push(Line::new(std::mem::replace(&mut curr_line, Vec::new())));
                    // we need to push current char to new content
                    new_content.push(c);
                    curr_width = c.append_width(0, cjk);
                }
            }
            // concat last span to curr_line
            if !new_content.is_empty() {
                let new_span = Span::new(new_content, new_style);
                curr_line.push(new_span);
            }
        }
    }
    if !curr_line.is_empty() {
        lines.push(Line::new(curr_line));
    }
}

// 将span合并进行，返回行是否结束
fn append_span(line: &mut Vec<Span>, span: Span) {
    if let Some(last_span) = line.last_mut() {
        if last_span.style == span.style {
            // 仅当格式相同时合并
            last_span.push_str(span.content);
            return;
        }
    }
    line.push(span);
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::ui::style::{Color, Style};

    #[test]
    fn test_wrap_single_line() {
        let line = Line::new(vec![ended_span("helloworld")]);
        let wl = line.wrap(20, true);
        assert_eq!(
            wl,
            WrapLine(vec![Line::new(vec![ended_span("helloworld")]),])
        );

        let line = Line::new(vec![ended_span("helloworld")]);
        let wl = line.wrap(4, true);
        assert_eq!(
            wl,
            WrapLine(vec![
                Line::new(vec![partial_span("hell")]),
                Line::new(vec![partial_span("owor")]),
                Line::new(vec![ended_span("ld")]),
            ])
        );

        let line = Line::new(vec![ended_span("中国人")]);
        let wl = line.wrap(3, true);
        assert_eq!(
            wl,
            WrapLine(vec![
                Line::new(vec![partial_span("中")]),
                Line::new(vec![partial_span("国")]),
                Line::new(vec![ended_span("人")]),
            ])
        )
    }

    #[test]
    fn test_wrap_multi_line() {
        let line = Line::new(vec![ended_span("helloworld")]);
        let wl = line.wrap(4, true);
        let wl = wl.reshape(6, true);
        assert_eq!(
            wl,
            WrapLine(vec![
                Line::new(vec![partial_span("hellow")]),
                Line::new(vec![ended_span("orld")]),
            ])
        );
    }

    #[test]
    fn test_warp_multi_style() {
        let line = Line::new(vec![red_span("hello"), ended_span("world")]);
        let wl = line.wrap(4, true);
        assert_eq!(
            wl,
            WrapLine(vec![
                Line::new(vec![red_span("hell")]),
                Line::new(vec![red_span("o"), partial_span("wor")]),
                Line::new(vec![ended_span("ld")]),
            ])
        );
    }

    fn ended_span(s: &str) -> Span {
        let mut s = s.to_owned();
        s.push_str("\r\n");
        Span::new(s, Style::default())
    }

    fn partial_span(s: &str) -> Span {
        Span::new(s, Style::default())
    }

    fn red_span(s: &str) -> Span {
        Span::new(s, Style::default().fg(Color::Red))
    }
}
