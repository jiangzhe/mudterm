use crate::error::Result;
use crate::ui::buffer::Buffer;
use crate::ui::layout::Rect;
use crate::ui::style::Style;
use crate::ui::widget::border;
use crate::ui::widget::Widget;
use crate::ui::width::AppendWidthTab8;

/// currently only support cjk mode
pub struct CmdBar {
    cmd: String,
    style: Style,
}

impl CmdBar {
    pub fn new() -> Self {
        Self {
            cmd: String::new(),
            style: Style::default(),
        }
    }

    pub fn cursor_pos(&self, area: Rect, cjk: bool) -> (u16, u16) {
        let width = if cjk { 2 } else { 1 };
        let offset = self.cmd.append_width(width, cjk) as u16;
        (area.left() + offset, area.top() + 1)
    }

    pub fn push_char(&mut self, ch: char) {
        self.cmd.push(ch);
    }

    pub fn push_str(&mut self, s: impl AsRef<str>) {
        self.cmd.push_str(s.as_ref());
    }

    pub fn take_cmd(&mut self) -> String {
        std::mem::replace(&mut self.cmd, String::new())
    }

    pub fn clear_cmd(&mut self) {
        self.cmd.clear();
    }
}

impl Widget for CmdBar {
    fn refresh_buffer<B: Buffer>(&mut self, buf: &mut B, cjk: bool) -> Result<()> {
        border::Border::Square.refresh_buffer(buf, cjk)?;
        let area = border::inner_area(*buf.area(), cjk);
        buf.set_line_str(
            area.left(),
            area.top(),
            &self.cmd,
            area.right(),
            self.style,
            cjk,
        );
        Ok(())
    }
}
