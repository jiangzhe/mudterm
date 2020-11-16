use crate::error::Result;
use crate::ui::buffer::Buffer;
use crate::ui::layout::Rect;
use crate::ui::style::Style;
use crate::ui::widget::{Block, Widget};
use crate::ui::width::AppendWidthTab8;

#[derive(Debug, Clone, PartialEq)]
pub enum CmdOut {
    Cmd(String),
    Script(String),
}

/// currently only support cjk mode
#[derive(Debug)]
pub struct CmdBar {
    cmd: String,
    block: Block,
    style: Style,
    script_mode: bool,
    script_prefix: char,
    cjk: bool,
}

impl CmdBar {
    pub fn new(script_prefix: char, cjk: bool) -> Self {
        Self {
            cmd: String::new(),
            block: Block::default().cjk(cjk),
            style: Style::default(),
            script_mode: false,
            script_prefix,
            cjk,
        }
    }

    pub fn cursor_pos(&self, area: Rect, cjk: bool) -> (u16, u16) {
        let width = if cjk { 2 } else { 1 };
        let offset = self.cmd.append_width(width, cjk) as u16;
        (area.left() + offset, area.top() + 1)
    }

    pub fn push_char(&mut self, ch: char) {
        if !self.script_mode && self.cmd.is_empty() && ch == self.script_prefix {
            self.script_mode = true;
        } else {
            self.cmd.push(ch);
        }
    }

    pub fn pop_char(&mut self) -> Option<char> {
        let ch = self.cmd.pop();
        if ch.is_some() && self.cmd.is_empty() {
            self.script_mode = false;
        }
        ch
    }

    pub fn take(&mut self) -> CmdOut {
        let s = std::mem::replace(&mut self.cmd, String::new());
        if self.script_mode {
            self.script_mode = false;
            CmdOut::Script(s)
        } else {
            CmdOut::Cmd(s)
        }
    }

    pub fn clear(&mut self) {
        self.cmd.clear();
    }
}

impl Widget for CmdBar {
    fn refresh_buffer<B: Buffer>(&mut self, buf: &mut B) -> Result<()> {
        self.block.refresh_buffer(buf)?;

        let bararea = self.block.inner_area(*buf.area());
        buf.set_line_str(
            bararea.left(),
            bararea.top(),
            &self.cmd,
            bararea.right(),
            self.style,
            self.cjk,
        );
        Ok(())
    }
}
