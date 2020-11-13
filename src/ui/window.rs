
use crate::conf;
use crate::error::Result;
use crate::ui::line::{WrapLine, Line, RawLine};
use crate::ui::style::Style;
use crate::ui::span::Span;
use std::collections::VecDeque;
use std::io::Write;
use termion::event::{Key, MouseButton, MouseEvent};
use termion::terminal_size;
use crate::ui::ansi::AnsiParser;
use crossbeam_channel::Receiver;

const HORIZONTAL: &str = "─";

pub struct Window {
    pub cmd: String,
    pub script_prefix: char,
    pub script_mode: bool,
    pub flow: Flow,

}

impl Window {
    pub fn new(width: usize, height: usize, termconf: conf::Term) -> Self {
        Self {
            cmd: String::new(),
            script_prefix: '.',
            script_mode: false,
            flow: Flow::new(width, height, termconf.max_lines),
        }
    }

    fn push_line(&mut self, line: RawLine) {
        self.flow.push_line(line);
    }

    fn push_lines(&mut self, lines: impl IntoIterator<Item=RawLine>) {
        self.flow.push_lines(lines)
    }

    fn draw_cmdline<W: Write>(&mut self, terminal: &mut W) -> Result<()> {
        self.script_mode(terminal)?;
        write!(terminal, "{}", self.cmd)?;
        Ok(())
    }

    fn script_mode<W: Write>(&self, terminal: &mut W) -> Result<()> {
        if self.script_mode {
            write!(terminal, "{}", termion::color::Bg(termion::color::Blue))?;
        } else {
            write!(terminal, "{}", termion::color::Bg(termion::color::Reset))?;
        }
        write!(terminal, "{}", termion::clear::CurrentLine)?;
        Ok(())
    }

    fn next_lines(&mut self) -> (Vec<RawLine>, bool) {
        self.flow.next_lines()
    }

    pub fn draw<W: Write>(&mut self, terminal: &mut W) -> Result<()> {
        let (lines, partial_ready) = self.next_lines();
        write!(terminal, "\r{}", termion::clear::CurrentLine)?;
        write!(terminal, "{}{}", termion::cursor::Up(1), termion::clear::CurrentLine)?;
        if partial_ready {
            write!(terminal, "{}{}", termion::cursor::Up(1), termion::clear::CurrentLine)?;
        }
        for line in lines {
            write!(terminal, "{}", line.as_ref())?;
        }
        // 空一行到命令行
        write!(terminal, "{}\r\n\r\n", termion::style::Reset)?;
        // 命令行
        self.draw_cmdline(terminal)?;
        Ok(())
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
                        let script = std::mem::replace(&mut self.cmd, String::new());
                        self.script_mode = false;
                        cb.on_script(self, script)
                    } else {
                        let cmd = std::mem::replace(&mut self.cmd, String::new());
                        cb.on_cmd(self, cmd);
                    }
                }
                Key::Char(c) if c == self.script_prefix && self.cmd.is_empty() => {
                    self.script_mode = true;
                    self.script_mode(terminal)?;
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
                        write!(terminal, "\r{}{}", termion::clear::CurrentLine, &self.cmd)?;
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
            // WindowEvent::Mouse(MouseEvent::Press(MouseButton::WheelUp, ..))
            //     if !self.auto_follow =>
            // {
            //     if self.scroll.0 > 0 {
            //         self.scroll.0 -= 1;
            //     }
            // }
            // WindowEvent::Mouse(MouseEvent::Press(MouseButton::WheelDown, ..))
            //     if !self.auto_follow =>
            // {
            //     // increase scroll means searching newer messages
            //     self.scroll.0 += 1;
            // }
            WindowEvent::Mouse(_) => {
                // not to render the screen
                return Ok(false);
            }
            WindowEvent::Tick | WindowEvent::WindowResize => (),
        }
        self.draw(terminal)?;
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
    // display: VecDeque<WrapLine>,
    // parser: AnsiParser,
    ready: usize,
    partial_ready: bool,
}

impl Flow {

    pub fn new(width: usize, height: usize, max_lines: usize) -> Self {
        debug_assert!(max_lines >= height);
        let mut raw = VecDeque::new();
        // let mut parsed = VecDeque::new();
        let empty_raw =  RawLine::owned(String::from("\r\n"));
        // let empty_parsed = WrapLine(vec![Line::fmt_raw("")]);
        for _ in 0..height {
            raw.push_back(empty_raw.clone());
            // parsed.push_back(empty_parsed.clone());
        }
        Self{
            width,
            height,
            max_lines,
            raw,
            // display: parsed,
            // parser: AnsiParser::new(),
            ready: height,
            partial_ready: false,
        }
    }

    pub fn push_line(&mut self, line: RawLine) {
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

    pub fn push_lines(&mut self, lines: impl IntoIterator<Item=RawLine>) {
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

    // pub fn wrap_lines(&self) -> Vec<WrapLine> {
    //     self.display.iter().cloned().collect()
    // }

    // fn push_span(&mut self, span: Span) {
    //     if let Some(last_line) = self.display.back_mut() {
    //         if !last_line.ended() {
    //             last_line.push_span(span, self.width, true);
    //             return;
    //         }
    //     }
    //     let line = Line::new(vec![span]);
    //     self.display.push_back(line.wrap(self.width, true));
    //     while self.display.len() > self.height {
    //         self.display.pop_front();
    //     }
    // }
}
