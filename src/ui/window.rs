use crate::conf;
use crate::error::Result;
use crate::ui::ansi::AnsiParser;
use crate::ui::line::{Line, RawLine, WrapLine};
use crate::ui::widget::Widget;
use crossbeam_channel::Receiver;
use std::collections::VecDeque;
use std::io::Write;
use termion::event::{Key, MouseButton, MouseEvent};

pub struct Window {
    cmd: String,
    script_prefix: char,
    script_mode: bool,
    flow: Flow,
}

// impl Widget for Window {
//     fn draw<W: Write>(&mut self, terminal: &mut W, cjk: bool) -> Result<()> {
//         let (lines, partial_ready) = self.next_lines();
//         if !partial_ready && lines.is_empty() {
//             return Ok(());
//         }
//         self.erase_cmdline(terminal)?;
//         if partial_ready {
//             write!(terminal, "{}{}", termion::cursor::Up(1), termion::clear::CurrentLine)?;
//         }
//         let last_end = lines.last().map(|l| l.ended()).unwrap_or(true);
//         for line in lines {
//             write!(terminal, "{}", line.as_ref())?;
//         }
//         if !last_end {
//             write!(terminal, "\r\n")?;
//         }
//         // 命令行
//         self.draw_cmdline(terminal)?;
//         Ok(())
//     }
// }

impl Window {
    pub fn new(width: usize, height: usize, termconf: conf::Term) -> Self {
        Self {
            cmd: String::new(),
            script_prefix: '.',
            script_mode: false,
            flow: Flow::new(width, height, termconf.max_lines),
        }
    }

    #[inline]
    pub fn push_line(&mut self, line: RawLine) {
        self.flow.push_line(line);
    }

    #[inline]
    pub fn push_lines(&mut self, lines: impl IntoIterator<Item = RawLine>) {
        self.flow.push_lines(lines)
    }

    #[inline]
    fn drain_cmd(&mut self) -> String {
        std::mem::replace(&mut self.cmd, String::new())
    }

    fn draw_cmdline<W: Write>(&mut self, terminal: &mut W) -> Result<()> {
        write!(terminal, "{}\r\n", termion::style::Reset)?;
        if self.script_mode {
            write!(terminal, "{}", termion::color::Bg(termion::color::Blue))?;
        } else {
            write!(terminal, "{}", termion::color::Bg(termion::color::Reset))?;
        }
        write!(terminal, "{}", termion::clear::CurrentLine)?;
        write!(terminal, "{}", self.cmd)?;
        Ok(())
    }

    fn erase_cmdline<W: Write>(&mut self, terminal: &mut W) -> Result<()> {
        // 当前行
        write!(
            terminal,
            "\r{}{}",
            termion::style::Reset,
            termion::clear::CurrentLine
        )?;
        // 上一行
        write!(
            terminal,
            "{}{}{}",
            termion::cursor::Up(1),
            termion::style::Reset,
            termion::clear::CurrentLine
        )?;
        Ok(())
    }

    fn next_lines(&mut self) -> (Vec<RawLine>, bool) {
        self.flow.next_lines()
    }

    pub fn render<W: Write, C: WindowCallback>(
        &mut self,
        terminal: &mut W,
        uirx: &Receiver<WindowEvent>,
        cb: &mut C,
    ) -> Result<bool> {
        match uirx.recv()? {
            WindowEvent::Key(key) => match key {
                Key::Char('\n') => {
                    if self.script_mode {
                        let script = self.drain_cmd();
                        self.script_mode = false;
                        cb.on_script(self, script)
                    } else {
                        let cmd = self.drain_cmd();
                        cb.on_cmd(self, cmd);
                    }
                }
                Key::Char(c) if c == self.script_prefix && self.cmd.is_empty() => {
                    self.script_mode = true;
                    self.draw_cmdline(terminal)?;
                    terminal.flush()?;
                    return Ok(false);
                }
                Key::Char(c) => {
                    self.cmd.push(c);
                    write!(terminal, "{}", c)?;
                    terminal.flush()?;
                    return Ok(false);
                }
                Key::Backspace => {
                    if self.cmd.pop().is_some() {
                        if self.cmd.is_empty() {
                            // turn off script mode
                            self.script_mode = false;
                        }
                        write!(
                            terminal,
                            "\r{}{}",
                            termion::style::Reset,
                            termion::clear::CurrentLine
                        )?;
                        self.draw_cmdline(terminal)?;
                        terminal.flush()?;
                    }
                    return Ok(false);
                }
                Key::Ctrl('q') => {
                    cb.on_quit(self);
                    return Ok(true);
                }
                Key::Ctrl('f') => {
                    // self.auto_follow = !self.auto_follow;
                }
                k => {
                    eprintln!("unhandled key {:?}", k);
                }
            },
            WindowEvent::Lines(lines) => self.push_lines(lines),
            WindowEvent::Line(line) => self.push_line(line),
            WindowEvent::Mouse(MouseEvent::Press(MouseButton::WheelUp, ..)) => {
                write!(terminal, "{}", termion::scroll::Down(1))?;
                terminal.flush()?;
                return Ok(false);
            }
            WindowEvent::Mouse(MouseEvent::Press(MouseButton::WheelDown, ..)) => {
                write!(terminal, "{}", termion::scroll::Up(1))?;
                terminal.flush()?;
                return Ok(false);
            }
            WindowEvent::Mouse(_) => {
                // not to render the screen
                return Ok(false);
            }
            WindowEvent::Tick | WindowEvent::WindowResize => (),
        }
        // self.draw(terminal, true)?;
        terminal.flush()?;
        Ok(false)
    }
}

pub trait WindowCallback {
    fn on_cmd(&mut self, window: &mut Window, cmd: String);

    fn on_script(&mut self, window: &mut Window, script: String);

    fn on_quit(&mut self, window: &mut Window);
}

#[derive(Debug, Clone)]
pub enum WindowEvent {
    Line(RawLine),
    Lines(Vec<RawLine>),
    Key(Key),
    Tick,
    WindowResize,
    Mouse(MouseEvent),
}

pub struct Flow {
    width: usize,
    height: usize,
    max_lines: usize,
    raw: VecDeque<RawLine>,
    display: VecDeque<WrapLine>,
    parser: AnsiParser,
    ready: usize,
    partial_ready: bool,
}

impl Flow {
    pub fn new(width: usize, height: usize, max_lines: usize) -> Self {
        debug_assert!(max_lines >= height);
        let mut raw = VecDeque::new();
        let mut parsed = VecDeque::new();
        let empty_raw = RawLine::owned(String::from("\r\n"));
        let empty_parsed = WrapLine(vec![Line::fmt_raw("")]);
        for _ in 0..height {
            raw.push_back(empty_raw.clone());
            parsed.push_back(empty_parsed.clone());
        }
        Self {
            width,
            height,
            max_lines,
            raw,
            display: parsed,
            parser: AnsiParser::new(),
            ready: height,
            partial_ready: false,
        }
    }

    pub fn push_line(&mut self, line: RawLine) {
        // 解析序列
        self.parse_ansi_line(line.content());

        // 原ansi字符序列
        if let Some(last_line) = self.raw.back_mut() {
            if !last_line.ended() {
                last_line.push_line(line);
                self.partial_ready = true;
                return;
            }
        }
        self.raw.push_back(line);
        self.ready += 1;
        while self.raw.len() > self.max_lines {
            self.raw.pop_front();
        }
    }

    fn parse_ansi_line(&mut self, line: impl AsRef<str>) {
        self.parser.fill(line.as_ref());
        while let Some(span) = self.parser.next_span() {
            let last_line = self.display.back_mut().unwrap();
            if !last_line.ended() {
                last_line.push_span(span, self.width, true);
            } else {
                let line = Line::single(span);
                let wl = line.wrap(self.width, true);
                self.display.push_back(wl);
            }
        }
        while self.display.len() > self.max_lines {
            self.display.pop_front();
        }
    }

    pub fn push_lines(&mut self, lines: impl IntoIterator<Item = RawLine>) {
        for line in lines {
            self.push_line(line);
        }
    }

    pub fn next_lines(&mut self) -> (Vec<RawLine>, bool) {
        let partial_ready = self.partial_ready;
        let mut ready = self.ready;
        if partial_ready {
            ready += 1;
        }
        let lines = self.raw.iter().rev().take(ready).cloned().rev().collect();
        self.partial_ready = false;
        self.ready = 0;
        (lines, partial_ready)
    }

    pub fn replay_lines(&mut self) -> Vec<RawLine> {
        let lines = self
            .raw
            .iter()
            .rev()
            .take(self.height)
            .cloned()
            .rev()
            .collect();
        self.partial_ready = false;
        self.ready = 0;
        lines
    }

    pub fn line_by_offset(&self, offset: usize) -> Option<RawLine> {
        self.raw.iter().rev().nth(offset).cloned()
    }
}
