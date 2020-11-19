use crate::error::Result;
use crate::ui::buffer::Buffer;
use crate::ui::layout::Rect;
use crate::ui::style::{Color, Style};
use crate::ui::widget::{Block, Widget};
use crate::ui::width::AppendWidthTab8;
use crate::ui::UserOutput;
use std::collections::VecDeque;

/// currently only support cjk mode
#[derive(Debug)]
pub struct CmdBar {
    cmd: UserOutput,
    block: Block,
    style: Style,
    script_prefix: char,
    cjk: bool,
    hist: CmdHist,
}

impl CmdBar {
    pub fn new(script_prefix: char, cjk: bool, hist_size: usize) -> Self {
        Self {
            cmd: UserOutput::default(),
            block: Block::default().cjk(cjk),
            style: Style::default(),
            // script_mode: false,
            script_prefix,
            cjk,
            hist: CmdHist::with_capacity(hist_size),
        }
    }

    pub fn cursor_pos(&self, area: Rect, cjk: bool) -> (u16, u16) {
        let width = if cjk { 2 } else { 1 };
        let offset = self.cmd.append_width(width, cjk) as u16;
        (area.left() + offset, area.top() + 1)
    }

    pub fn push_char(&mut self, ch: char) {
        if self.cmd.is_empty() && ch == self.script_prefix {
            if self.cmd.is_cmd() {
                self.cmd = UserOutput::Script(String::new());
                self.style = Style::default().bg(Color::Blue);
            } else {
                self.cmd = UserOutput::Cmd(String::new());
                self.style = Style::default();
            }
        } else {
            self.cmd.push(ch);
        }
    }

    pub fn pop_char(&mut self) -> Option<char> {
        let ch = self.cmd.pop();
        if ch.is_some() && self.cmd.is_empty() && self.cmd.is_script() {
            self.style = Style::default();
            self.cmd = UserOutput::Cmd(String::new());
        }
        ch
    }

    pub fn take(&mut self) -> UserOutput {
        let cmd = std::mem::replace(&mut self.cmd, UserOutput::default());
        // 每次都记录历史
        self.hist.push(cmd.clone());
        self.style = Style::default();
        cmd
    }

    pub fn clear(&mut self) {
        self.cmd.clear();
    }

    pub fn prev_cmd(&mut self) {
        if let Some(prev) = self.hist.prev() {
            self.cmd = prev.clone();
        }
    }

    pub fn next_cmd(&mut self) {
        if let Some(next) = self.hist.next() {
            self.cmd = next.clone();
        }
    }
}

impl Widget for CmdBar {
    fn refresh_buffer<B: Buffer>(&mut self, buf: &mut B) -> Result<()> {
        self.block.refresh_buffer(buf)?;

        let bararea = self.block.inner_area(*buf.area());
        buf.set_style(bararea, self.style);
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

#[derive(Debug)]
struct CmdHist {
    cmds: VecDeque<UserOutput>,
    idx: usize,
    capacity: usize,
}

impl CmdHist {
    /// 指定容量，创建命令历史记录
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            cmds: VecDeque::with_capacity(capacity),
            idx: 0,
            capacity,
        }
    }

    /// 后移并获取命令
    pub fn next(&mut self) -> Option<&UserOutput> {
        if self.idx >= self.len() - 1 {
            return None;
        }
        if self.idx < self.len() - 1 {
            self.idx += 1;
        }
        self.cmds.get(self.idx)
    }

    /// 前移并获取命令
    pub fn prev(&mut self) -> Option<&UserOutput> {
        if self.idx == 0 {
            return None;
        }
        if self.idx > 0 {
            self.idx -= 1;
        }
        return self.cmds.get(self.idx);
    }

    pub fn clear(&mut self) {
        self.cmds.clear();
        self.idx = 0;
    }

    pub fn push(&mut self, cmd: UserOutput) {
        if let Some(last) = self.last() {
            // 与上一命令完全相同，忽略
            if &cmd == last {
                return;
            }
        }
        if self.cmds.len() == self.capacity {
            self.cmds.pop_front();
        }
        self.cmds.push_back(cmd.into());
        self.idx = self.cmds.len();
    }

    pub fn len(&self) -> usize {
        self.cmds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cmds.len() == 0
    }

    pub fn first(&self) -> Option<&UserOutput> {
        self.cmds.front()
    }

    pub fn last(&self) -> Option<&UserOutput> {
        self.cmds.back()
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_cmd_hist() {
        let mut hist = CmdHist::with_capacity(3);
        assert!(hist.is_empty());
        hist.push(UserOutput::Cmd("hello".into()));
        assert!(!hist.is_empty());
        assert_eq!(&UserOutput::Cmd("hello".into()), hist.prev().unwrap());
        hist.push(UserOutput::Cmd("world".into()));
        assert_eq!(&UserOutput::Cmd("world".into()), hist.prev().unwrap());
        assert_eq!(&UserOutput::Cmd("hello".into()), hist.prev().unwrap());
        assert!(hist.prev().is_none());
        hist.push(UserOutput::Cmd("java".into()));
        assert_eq!(&UserOutput::Cmd("java".into()), hist.prev().unwrap());
        assert_eq!(&UserOutput::Cmd("hello".into()), hist.first().unwrap());
        assert_eq!(&UserOutput::Cmd("java".into()), hist.last().unwrap());
        hist.push(UserOutput::Cmd("overflow".into()));
        assert_eq!(&UserOutput::Cmd("world".into()), hist.first().unwrap());
        hist.push(UserOutput::Cmd("overflow".into()));
        assert_eq!(&UserOutput::Cmd("world".into()), hist.first().unwrap());
    }
}
