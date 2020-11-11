use std::sync::Arc;
use tui::style::Style;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// 与tui::text::Span相似，可以在线程间传递
#[derive(Debug, Clone, PartialEq)]
pub struct ArcSpan {
    content: Arc<str>,
    pub style: Style,
    pub ended: bool,
}

impl ArcSpan {

    pub fn new(content: impl Into<String>, style: Style, ended: bool) -> Self {
        Self{
            content: Arc::from(content.into()),
            style,
            ended,
        }
    }

    pub fn deep_copy(&self) -> Self {
        Self::new(self.content.as_ref().to_owned(), self.style, self.ended)
    }

    #[inline]
    pub fn content(&self) -> &str {
        self.content.as_ref()
    }

    #[inline]
    pub fn width(&self, cjk: bool) -> usize {
        if cjk { self.content.width_cjk() } else { self.content.width() }
    }

    // 需要拷贝原字符串，但目前场景下并不常见
    #[inline]
    pub fn push_str(&mut self, s: &str) {
        let mut content = self.content.as_ref().to_owned();
        content.push_str(s);
        self.content = Arc::from(content);
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
            for c in span.content.chars() {
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
    use tui::style::Color;

    #[test]
    fn test_invisible_chars() {
        println!("0x21 width={}", '\x21'.width_cjk().unwrap_or(0));
    }

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
