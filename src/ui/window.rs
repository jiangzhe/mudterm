
use crate::conf;
use crate::error::Result;
use crate::ui::line::{WrapLine, Line, RawLine};
use crate::ui::style::Style;
use crate::ui::span::ArcSpan;
use std::collections::VecDeque;
use std::io::Write;
use termion::event::{Key, MouseButton, MouseEvent};
use termion::terminal_size;
use crate::ui::ansi::SpanStream;

const HORIZONTAL: &str = "─";

pub struct Window {
    pub width: usize,
    pub height: usize,
    pub cmd: String,
    pub script_prefix: char,
    pub script_mode: bool,
    pub flow: Flow,
}

impl Window {
    pub fn new(width: usize, height: usize, termconf: conf::Term) -> Self {
        Self {
            width,
            height,
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

    // pub fn draw<W: Write>(&mut self, terminal: &mut W) -> Result<()> {
    //     let (_, height) = terminal_size()?;
    //     let height = height as usize;
    //     if height > self.lines.len() {
    //         for _ in self.lines.len()..height {
    //             write!(terminal, "\r\n")?;
    //         }
    //     }
    //     for line in self.lines.iter().rev().take(height).rev() {
    //         write!(terminal, "{}", line.as_ref())?;
    //         // to remove
    //         // screen.spans.fill(line.as_ref());
    //         // let mut width_cjk = 0;
    //         // let mut width = 0;
    //         // while let Some(span) = screen.spans.next_span() {
    //         //     width_cjk += span.width(true);
    //         //     width += span.width(false);
    //         //     eprint!("{}", line.as_ref());
    //         //     eprintln!("width={}, width_cjk={}", width, width_cjk);
    //         // }
    //     }
    //     // 空一行到命令行
    //     write!(terminal, "{}\r\n\r\n", termion::style::Reset)?;
    //     // 命令行
    //     self.draw_cmdline(terminal)?;
    //     Ok(())
    // }
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
    parsed: VecDeque<WrapLine>,
    parser: SpanStream,
}

impl Flow {

    pub fn new(width: usize, height: usize, max_lines: usize) -> Self {
        debug_assert!(max_lines >= height);
        let mut raw = VecDeque::new();
        let mut parsed = VecDeque::new();
        let empty_raw =  RawLine::owned(String::from("\r\n"));
        let empty_parsed = WrapLine(vec![Line::fmt_raw("")]);
        for _ in 0..height {
            raw.push_back(empty_raw.clone());
            parsed.push_back(empty_parsed.clone());
        }
        Self{
            width,
            height,
            max_lines,
            raw,
            parsed,
            parser: SpanStream::new(),
        }
    }

    pub fn push_line(&mut self, line: RawLine) {
        self.parser.fill(line.content());
        while let Some(span) = self.parser.next_span() {
            self.push_span(span);
        }
        self.push_raw(line);
    }

    pub fn push_lines(&mut self, lines: impl IntoIterator<Item=RawLine>) {
        for line in lines {
            self.push_line(line);
        }
    }

    fn push_raw(&mut self, line: RawLine) {
        if let Some(last_line) = self.raw.back_mut() {
            if !last_line.ended() {
                last_line.push_line(line);
                return;
            }
        }
        self.raw.push_back(line);
        while self.raw.len() > self.max_lines {
            self.raw.pop_front();
        }
    }

    fn push_span(&mut self, span: ArcSpan) {
        if let Some(last_line) = self.parsed.back_mut() {
            if !last_line.ended() {
                last_line.push_span(span, self.width, true);
                return;
            }
        }
        let line = Line::new(vec![span]);
        self.parsed.push_back(line.wrap(self.width, true));
        while self.parsed.len() > self.height {
            self.parsed.pop_front();
        }
    }
}
