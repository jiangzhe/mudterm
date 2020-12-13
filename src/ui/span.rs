use crate::ui::style::{Color, Modifier, Style};
use crate::proto::Label;

/// 与tui::text::Span相似，可以在线程间传递
#[derive(Clone)]
pub struct Span {
    pub style: Style,
    pub content: String,
    pub label: Label,
}

impl PartialEq for Span {
    fn eq(&self, other: &Self) -> bool {
        self.label == other.label &&
        self.style == other.style && 
        self.content == other.content
    }
}

impl std::fmt::Debug for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "\"{}\"", self.content)
    }
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.style, self.content)
    }
}

impl Span {
    pub fn new(content: impl Into<String>, style: Style, label: Label) -> Self {
        let content = content.into();
        Self { style, content, label }
    }

    pub fn fmt_raw(content: impl Into<String>) -> Self {
        Self::fmt_with_style(content, Style::default())
    }

    pub fn fmt_note(content: impl Into<String>) -> Self {
        Self::fmt_with_style(content, Style::default().fg(Color::LightBlue))
    }

    pub fn fmt_with_style(content: impl Into<String>, style: Style) -> Self {
        let mut content = content.into();
        if !content.ends_with("\r\n") {
            content.push_str("\r\n");
        }
        Self::new(content, style, Label::None)
    }

    pub fn fmt_err(content: impl Into<String>) -> Self {
        Self::fmt_with_style(content, Style::default().add_modifier(Modifier::REVERSED))
    }

    #[inline]
    pub fn ended(&self) -> bool {
        self.content.ends_with('\n')
    }

    #[inline]
    pub fn push_str(&mut self, s: impl AsRef<str>) {
        self.content.push_str(s.as_ref());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_str_to_span() {
        let s = String::from("hello中国");
        let mut span = Span::new(s, Style::default(), Label::None);
        println!("span={}", span);
        span.push_str("你好");
        println!("span={}", span);
    }
}
