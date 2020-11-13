use crate::ui::style::{Color, Modifier, Style};
use std::sync::Arc;

/// 与tui::text::Span相似，可以在线程间传递
#[derive(Clone)]
pub struct Span {
    pub style: Style,
    orig: Arc<str>,
    start: usize,
    end: usize,
}

impl PartialEq for Span {
    fn eq(&self, other: &Self) -> bool {
        self.style == other.style && self.content() == other.content()
    }
}

impl std::fmt::Debug for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "\"{}\"", self.content())
    }
}

// impl std::fmt::Display for Span {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        
//     }
// }

impl Span {

    pub fn new(content: impl Into<String>, style: Style) -> Self {
        let content = content.into();
        let start = 0;
        let end = content.len();
        Self {
            orig: Arc::from(content),
            style,
            start,
            end,
        }
    }

    pub fn borrowed(orig: Arc<str>, start: usize, end: usize, style: Style) -> Self {
        Self{
            orig,
            style,
            start,
            end,
        }
    }

    pub fn fmt_raw(content: impl Into<String>) -> Self {
        let mut content = content.into();
        content.push_str("\r\n");
        Self::new(content, Style::default())
    }

    pub fn fmt_note(content: impl Into<String>) -> Self {
        let mut content = content.into();
        content.push_str("\r\n");
        Self::new(content, Style::default().fg(Color::LightBlue))
    }

    pub fn fmt_err(content: impl Into<String>) -> Self {
        let mut content = content.into();
        content.push_str("\r\n");
        Self::new(content, Style::default().add_modifier(Modifier::REVERSED))
    }

    #[inline]
    pub fn content(&self) -> &str {
        &self.orig.as_ref()[self.start..self.end]
    }

    #[inline]
    pub fn ended(&self) -> bool {
        self.content().ends_with('\n')
    }

    // 需要拷贝原字符串，但目前场景下并不常见
    #[inline]
    pub fn push_str(&mut self, s: &str) {
        let mut new = self.content().to_owned();
        new.push_str(s);
        self.orig = Arc::from(new);
    }
}
