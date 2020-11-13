// pub mod flow;
pub mod ansi;
pub mod line;
pub mod span;
pub mod style;
pub mod terminal;
pub mod window;
pub mod width;

use crate::conf;
use crate::error::Result;
use crossbeam_channel::Receiver;
use line::RawLine;
use std::collections::VecDeque;
use std::io::Write;
use termion::event::{Key, MouseButton, MouseEvent};
use termion::terminal_size;
use crate::ui::ansi::SpanStream;

pub struct RawScreen {
    // pub flow: MessageFlow,
    pub lines: VecDeque<RawLine>,
    pub max_lines: usize,
    pub cmd: String,
    pub script_prefix: char,
    pub script_mode: bool,
    pub auto_follow: bool,
    pub scroll: (u16, u16),
    pub spans: SpanStream,
}

impl RawScreen {
    pub fn new(termconf: conf::Term) -> Self {
        // let flow = MessageFlow::new()
        //     .max_lines(termconf.max_lines as u32)
        //     .cjk(true);
        Self {
            lines: VecDeque::new(),
            max_lines: termconf.max_lines,
            cmd: String::new(),
            script_prefix: '.',
            script_mode: false,
            auto_follow: true,
            scroll: (0, 0),
            spans: SpanStream::new(),
        }
    }

    fn push_line(&mut self, line: RawLine) {
        if let Some(last_line) = self.lines.back_mut() {
            if !last_line.ended() {
                last_line.push_line(&line);
                return;
            }
        }
        self.lines.push_back(line);
    }

    fn push_lines(&mut self, lines: Vec<RawLine>) {
        for line in lines {
            self.push_line(line);
        }
    }

    /// returns true means quit the render process
    pub fn render<W: Write, C: RawScreenCallback>(
        &mut self,
        terminal: &mut W,
        uirx: &Receiver<RawScreenInput>,
        cb: &mut C,
    ) -> Result<bool> {
        match uirx.recv()? {
            RawScreenInput::Key(key) => match key {
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
                    script_mode(terminal, self.script_mode)?;
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
                    self.auto_follow = !self.auto_follow;
                }
                k => {
                    eprintln!("unhandled key {:?}", k);
                }
            },
            RawScreenInput::Lines(lines) => self.push_lines(lines),
            RawScreenInput::Line(line) => self.push_line(line),
            RawScreenInput::Mouse(MouseEvent::Press(MouseButton::WheelUp, ..))
                if !self.auto_follow =>
            {
                if self.scroll.0 > 0 {
                    self.scroll.0 -= 1;
                }
            }
            RawScreenInput::Mouse(MouseEvent::Press(MouseButton::WheelDown, ..))
                if !self.auto_follow =>
            {
                // increase scroll means searching newer messages
                self.scroll.0 += 1;
            }
            RawScreenInput::Mouse(_) => {
                // not to render the screen
                return Ok(false);
            }
            RawScreenInput::Tick | RawScreenInput::WindowResize => (),
        }
        draw_terminal(self, terminal)?;
        terminal.flush()?;
        Ok(false)
    }
}

fn draw_cmdline<W: Write>(screen: &mut RawScreen, terminal: &mut W) -> Result<()> {
    script_mode(terminal, screen.script_mode)?;
    write!(terminal, "{}", screen.cmd)?;
    Ok(())
}

fn script_mode<W: Write>(terminal: &mut W, script_mode: bool) -> Result<()> {
    if script_mode {
        write!(terminal, "{}", termion::color::Bg(termion::color::Blue))?;
    } else {
        write!(terminal, "{}", termion::color::Bg(termion::color::Reset))?;
    }
    write!(terminal, "{}", termion::clear::CurrentLine)?;
    Ok(())
}

fn draw_terminal<W: Write>(screen: &mut RawScreen, terminal: &mut W) -> Result<()> {
    let (_, height) = terminal_size()?;
    let height = height as usize;
    if height > screen.lines.len() {
        for _ in screen.lines.len()..height {
            write!(terminal, "\r\n")?;
        }
    }
    for line in screen.lines.iter().rev().take(height).rev() {
        write!(terminal, "{}", line.as_ref())?;
        // to remove
        // screen.spans.fill(line.as_ref());
        // let mut width_cjk = 0;
        // let mut width = 0;
        // while let Some(span) = screen.spans.next_span() {
        //     width_cjk += span.width(true);
        //     width += span.width(false);
        //     eprint!("{}", line.as_ref());
        //     eprintln!("width={}, width_cjk={}", width, width_cjk);
        // }
    }
    // 空一行到命令行
    write!(terminal, "{}\r\n\r\n", termion::style::Reset)?;
    // 命令行
    draw_cmdline(screen, terminal)?;
    Ok(())
}

pub trait RawScreenCallback {
    fn on_cmd(&mut self, screen: &mut RawScreen, cmd: String);

    fn on_script(&mut self, screen: &mut RawScreen, script: String);

    fn on_quit(&mut self, screen: &mut RawScreen);
}


#[derive(Debug, Clone)]
pub enum RawScreenInput {
    // Line(StyledLine),
    Line(RawLine),
    // Lines(VecDeque<StyledLine>),
    Lines(Vec<RawLine>),
    Key(Key),
    Tick,
    WindowResize,
    Mouse(MouseEvent),
}
