use crate::error::Result;
use crate::ui::buffer::Buffer;
use crate::ui::layout::Rect;
use crate::ui::style::Style;
use crate::ui::widget::{Block, Widget};
use crate::ui::width::AppendWidthTab8;
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq)]
pub enum Output {
    Cmd(String),
    Script(String),
}

impl Default for Output {
    fn default() -> Self {
        Self::Cmd(String::new())
    }
}

impl AsRef<str> for Output {
    fn as_ref(&self) -> &str {
        match self {
            Self::Cmd(s) => s,
            Self::Script(s) => s,
        }
    }
}

impl Output {
    pub fn push(&mut self, c: char) {
        match self {
            Self::Cmd(s) => s.push(c),
            Self::Script(s) => s.push(c),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self::Cmd(s) => s.is_empty(),
            Self::Script(s) => s.is_empty(),
        }
    }

    pub fn is_cmd(&self) -> bool {
        match self {
            Self::Cmd(_) => true,
            _ => false,
        }
    }

    pub fn is_script(&self) -> bool {
        match self {
            Self::Script(_) => true,
            _ => false,
        }
    }

    pub fn pop(&mut self) -> Option<char> {
        match self {
            Self::Cmd(s) => s.pop(),
            Self::Script(s) => s.pop(),
        }
    }

    pub fn clear(&mut self) {
        *self = Output::default();
    }
}

impl AppendWidthTab8 for Output {
    fn append_width(&self, prev_width: usize, cjk: bool) -> usize {
        match self {
            Self::Cmd(s) => s.append_width(prev_width, cjk),
            Self::Script(s) => s.append_width(prev_width, cjk),
        }
    }
}

/// currently only support cjk mode
#[derive(Debug)]
pub struct CmdBar {
    cmd: Output,
    block: Block,
    style: Style,
    // script_mode: bool,
    script_prefix: char,
    cjk: bool,
    hist: CmdHist,
}

impl CmdBar {
    pub fn new(script_prefix: char, cjk: bool, hist_size: usize) -> Self {
        Self {
            cmd: Output::default(),
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
            self.cmd = Output::Script(String::new());
        } else {
            self.cmd.push(ch);
        }
    }

    pub fn pop_char(&mut self) -> Option<char> {
        let ch = self.cmd.pop();
        if ch.is_some() && self.cmd.is_empty() && self.cmd.is_script() {
            self.cmd = Output::Cmd(String::new());
        }
        ch
    }

    pub fn take(&mut self) -> Output {
        let cmd = std::mem::replace(&mut self.cmd, Output::default());
        // 每次都记录历史
        self.hist.push(cmd.clone());
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
    cmds: VecDeque<Output>,
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
    pub fn next(&mut self) -> Option<&Output> {
        if self.idx >= self.len() - 1 {
            return None;
        }
        if self.idx < self.len() - 1 {
            self.idx += 1;
        }
        self.cmds.get(self.idx)
    }

    /// 前移并获取命令
    pub fn prev(&mut self) -> Option<&Output> {
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

    pub fn push(&mut self, cmd: Output) {
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

    pub fn first(&self) -> Option<&Output> {
        self.cmds.front()
    }

    pub fn last(&self) -> Option<&Output> {
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
        hist.push(Output::Cmd("hello".into()));
        assert!(!hist.is_empty());
        assert_eq!(&Output::Cmd("hello".into()), hist.prev().unwrap());
        hist.push(Output::Cmd("world".into()));
        assert_eq!(&Output::Cmd("world".into()), hist.prev().unwrap());
        assert_eq!(&Output::Cmd("hello".into()), hist.prev().unwrap());
        assert!(hist.prev().is_none());
        hist.push(Output::Cmd("java".into()));
        assert_eq!(&Output::Cmd("java".into()), hist.prev().unwrap());
        assert_eq!(&Output::Cmd("hello".into()), hist.first().unwrap());
        assert_eq!(&Output::Cmd("java".into()), hist.last().unwrap());
        hist.push(Output::Cmd("overflow".into()));
        assert_eq!(&Output::Cmd("world".into()), hist.first().unwrap());
        hist.push(Output::Cmd("overflow".into()));
        assert_eq!(&Output::Cmd("world".into()), hist.first().unwrap());
    }
}
