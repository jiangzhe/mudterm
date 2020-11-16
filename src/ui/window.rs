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
