use std::collections::VecDeque;
use tui::text::Spans;
use crate::style::StyledLine;
use crate::error::Result;
use crate::conf;
use crossbeam_channel::Receiver;
use std::io;
use termion::raw::IntoRawMode;
use termion::input::MouseTerminal;
use termion::screen::AlternateScreen;
use termion::event::{Key, MouseEvent, MouseButton};
use tui::backend::{Backend, TermionBackend};
use tui::Terminal;
use tui::layout::{Layout, Direction, Constraint};
use tui::widgets::{Paragraph, Block, Borders, Wrap};
use tui::style::{Style, Color};


pub struct RawScreen {
    pub lines: Lines,
    pub command: String,
    pub script_prefix: char,
    pub script_mode: bool,
    pub auto_follow: bool,
    pub scroll: (u16, u16),
}

impl RawScreen {

    pub fn new(termconf: conf::Term) -> Self {
        let mut lines = Lines::new();
        lines.set_max_lines(termconf.max_lines);
        Self{
            lines,
            command: String::new(),
            script_prefix: '.',
            script_mode: false,
            auto_follow: true,
            scroll: (0, 0),
        }
    }

    pub fn render<B: Backend, C: RawScreenCallback>(mut self, mut terminal: Terminal<B>, uirx: Receiver<RawScreenInput>, mut cb: C) -> Result<()> {
        loop {
            match uirx.recv()? {
                RawScreenInput::Key(key) => match key {
                    Key::Char('\n') => {
                        if self.script_mode {
                            let mut script = std::mem::replace(&mut self.command, String::new());
                            script.remove(0);
                            self.script_mode = false;
                            cb.on_script(&mut self, script)
                        } else {
                            let cmd = std::mem::replace(&mut self.command, String::new());
                            cb.on_cmd(&mut self, cmd);
                        }
                    }
                    Key::Char(c) if c == self.script_prefix && self.command.is_empty() => {
                        self.command.push(c);
                        self.script_mode = true;
                    }
                    Key::Char(c) => {
                        self.command.push(c);
                    }
                    Key::Backspace => {
                        self.command.pop();
                        if self.command.is_empty() {
                            // turn off script mode
                            self.script_mode = false;
                        }
                    }
                    Key::Ctrl('q') => {
                        // self.evttx.send(Event::Quit)?;
                        cb.on_quit(&mut self);
                        break;
                    }
                    Key::Ctrl('f') => {
                        self.auto_follow = !self.auto_follow;
                    }
                    k => {
                        eprintln!("unhandled key {:?}", k);
                    },
                }
                RawScreenInput::Lines(sms) => {
                    self.lines.push_lines(sms);
                }
                RawScreenInput::Line(sm) => {
                    if !self.lines.is_last_line_ended() {
                        self.lines.append_to_last_line(sm);
                    } else {
                        self.lines.push_line(sm);
                    }
                }
                RawScreenInput::Mouse(MouseEvent::Press(MouseButton::WheelUp, ..)) if !self.auto_follow => {
                    if self.scroll.0 > 0 {
                        self.scroll.0 -= 1;
                    }
                }
                RawScreenInput::Mouse(MouseEvent::Press(MouseButton::WheelDown, ..)) if !self.auto_follow => {
                    // increase scroll means searching newer messages
                    self.scroll.0 += 1;
                }
                RawScreenInput::Mouse(_) => {
                    // not to render the screen
                    continue;
                },
                RawScreenInput::Tick | RawScreenInput::WindowResize => (),
            }
            terminal.draw(|f| {
                // with border
                let server_board_height = f.size().height as usize - 3;
                let server_board_width = f.size().width - 3;
                // let server_max_lines = server_board_height - 2;
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(0)
                    .constraints(
                        [
                            Constraint::Length(server_board_height as u16),
                            Constraint::Length(3),
                        ].as_ref(),
                    )
                    .split(f.size());
                // server board
                let text = self.lines.lastn_with_width(5000, server_board_width);
                if self.auto_follow {
                    if server_board_height >= text.len() + 2 {
                        self.scroll.0 = 0;
                    } else {
                        self.scroll.0 = (text.len() + 2 - server_board_height) as u16;
                    }
                }
                let paragraph = Paragraph::new(text)
                    .style(Style::default())
                    .scroll(self.scroll)
                    .block(Block::default().title("Server").borders(Borders::ALL));
                f.render_widget(paragraph, chunks[0]);
                // user input
                let mut cmd_style = Style::default();
                if self.script_mode {
                    cmd_style = cmd_style.bg(Color::Blue);
                }
                let cmd = Paragraph::new(self.command.as_ref())
                    .style(cmd_style)
                    .block(Block::default().borders(Borders::ALL).title("Command"))
                    .wrap(Wrap{trim:false});
                f.render_widget(cmd, chunks[1]);
            })?;
        }
        Ok(())
    }
}

pub trait RawScreenCallback {

    fn on_cmd(&mut self, term: &mut RawScreen, cmd: String);

    fn on_script(&mut self, term: &mut RawScreen, script: String);

    fn on_quit(&mut self, term: &mut RawScreen);
}

pub fn render_ui(term: RawScreen, uirx: Receiver<RawScreenInput>, cb: impl RawScreenCallback) -> Result<()> {
    let stdout = io::stdout().into_raw_mode()?;
    let stdout = MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    term.render(terminal, uirx, cb)
}

#[derive(Debug, Clone)]
pub enum RawScreenInput {
    Line(StyledLine),
    Lines(VecDeque<StyledLine>),
    Key(Key),
    Tick,
    WindowResize,
    Mouse(MouseEvent),
}

pub struct Lines {
    buffer: VecDeque<StyledLine>,
    max_lines: usize,
}

impl Lines {

    pub fn new() -> Self {
        Self{
            buffer: VecDeque::new(),
            max_lines: 5000,
        }
    }

    pub fn set_max_lines(&mut self, max_lines: usize) {
        self.max_lines = max_lines;
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_last_line_ended(&self) -> bool {
        if let Some(last_buf) = self.buffer.back() {
            return last_buf.ended;
        }
        true
    }

    pub fn append_to_last_line(&mut self, line: StyledLine) {
        let last_buf = self.buffer.back_mut().unwrap();
        last_buf.spans.extend(line.spans);
        last_buf.orig.push_str(&line.orig);
        last_buf.ended = line.ended;
    }

    pub fn push_line(&mut self, line: StyledLine) {
        if self.buffer.len() == self.max_lines {
            self.buffer.pop_front();
        }
        self.buffer.push_back(line);
    }

    pub fn push_lines(&mut self, mut lines: VecDeque<StyledLine>) {
        if lines.is_empty() {
            return;
        }
        let first_line = lines.pop_front().unwrap();
        if !self.is_last_line_ended() {
            self.append_to_last_line(first_line);
        } else {
            self.push_line(first_line);
        }
        for line in lines {
            self.push_line(line);
        }
    }

    pub fn lastn(&self, n: usize) -> Vec<Spans<'static>> {
        if self.buffer.len() <= n {
            self.buffer.iter()
                .map(|m| Spans::from(m.spans.clone()))
                .collect()
        } else {
            self.buffer.iter().skip(self.buffer.len() - n)
                .map(|m| Spans::from(m.spans.clone()))
                .collect()
        }
    }

    /// lines with larger width will be splited
    pub fn lastn_with_width(&self, n: usize, max_width: u16) -> Vec<Spans<'static>> {
        let mut iter = self.buffer.iter().cloned();
        let mut lineno = 0;
        let mut reversed = Vec::new();
        'outer: while let Some(sl) = iter.next_back() {
            let mut split_iter = sl.split_with_max_width(max_width).into_iter();
            while let Some(line) = split_iter.next_back() {
                reversed.push(Spans(line));
                lineno += 1;
                if lineno == n {
                    break 'outer;
                }
            }
        }
        reversed.into_iter().rev().collect()
    }
}
