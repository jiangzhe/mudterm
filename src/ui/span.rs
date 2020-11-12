use std::sync::Arc;
use tui::style::{Style, Color, Modifier};
use unicode_width::UnicodeWidthStr;

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

    pub fn raw(content: impl Into<String>) -> Self {
        Self{
            content: Arc::from(content.into()),
            style: Style::default(),
            ended: true,
        }
    }

    pub fn note(content: impl Into<String>) -> Self {
        Self {
            content: Arc::from(content.into()),
            style: Style::default().fg(Color::LightBlue),
            ended: true,
        }
    }

    pub fn err(content: impl Into<String>) -> Self {
        Self {
            content: Arc::from(content.into()),
            style: Style::default().add_modifier(Modifier::REVERSED),
            ended: true,
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
