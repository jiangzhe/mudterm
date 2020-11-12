pub mod flow;
pub mod line;
pub mod ansi;
pub mod span;

use crate::conf;
use crate::error::Result;
use line::Line;
use flow::{MessageFlow, FlowBoard};
use crossbeam_channel::Receiver;
use std::io::{self, Stdout};
use termion::event::{Key, MouseButton, MouseEvent};
use termion::input::MouseTerminal;
use termion::raw::{IntoRawMode, RawTerminal};
use termion::screen::AlternateScreen;
use tui::backend::{Backend, TermionBackend};
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Style};
use tui::widgets::{Block, Borders, Paragraph, Wrap};
use tui::Terminal;

pub struct RawScreen {
    // pub lines: Lines,
    pub flow: MessageFlow,
    pub command: String,
    pub script_prefix: char,
    pub script_mode: bool,
    pub auto_follow: bool,
    pub scroll: (u16, u16),
}

impl RawScreen {
    pub fn new(termconf: conf::Term) -> Self {
        let flow = MessageFlow::new()
            .max_lines(termconf.max_lines as u32)
            .cjk(true);
        Self {
            flow,
            command: String::new(),
            script_prefix: '.',
            script_mode: false,
            auto_follow: true,
            scroll: (0, 0),
        }
    }

    /// returns true means quit the render process
    pub fn render<B: Backend, C: RawScreenCallback>(
        &mut self,
        terminal: &mut Terminal<B>,
        uirx: &Receiver<RawScreenInput>,
        cb: &mut C,
    ) -> Result<bool> {
        match uirx.recv()? {
            RawScreenInput::Key(key) => match key {
                Key::Char('\n') => {
                    if self.script_mode {
                        let mut script = std::mem::replace(&mut self.command, String::new());
                        script.remove(0);
                        self.script_mode = false;
                        cb.on_script(self, script)
                    } else {
                        let cmd = std::mem::replace(&mut self.command, String::new());
                        cb.on_cmd(self, cmd);
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
            RawScreenInput::Lines(lines) => {
                self.flow.push_lines(lines);
            }
            RawScreenInput::Line(line) => {
                self.flow.push_line(line)
            }
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
        Ok(false)
    }
}

fn draw_terminal<B: Backend>(screen: &mut RawScreen, terminal: &mut Terminal<B>) -> Result<()> {
    terminal.draw(|f| {
        // with border
        let server_board_height = f.size().height as usize - 2;
        // let server_board_width = f.size().width - 3;
        // let server_max_lines = server_board_height - 2;
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints(
                [
                    Constraint::Length(server_board_height as u16),
                    Constraint::Length(2),
                ]
                .as_ref(),
            )
            .split(f.size());
        // server board
        let board = FlowBoard::new(&screen.flow, Block::default().title(" ").borders(Borders::NONE), Style::default());
        f.render_widget(board, chunks[0]);
        // user input
        let mut cmd_style = Style::default();
        if screen.script_mode {
            cmd_style = cmd_style.bg(Color::Blue);
        }
        let cmd = Paragraph::new(screen.command.as_ref())
            .style(cmd_style)
            .block(Block::default().borders(Borders::NONE).title(" "));
            // .wrap(Wrap { trim: false });
        f.render_widget(cmd, chunks[1]);
    })?;
    Ok(())
}

pub trait RawScreenCallback {
    fn on_cmd(&mut self, screen: &mut RawScreen, cmd: String);

    fn on_script(&mut self, screen: &mut RawScreen, script: String);

    fn on_quit(&mut self, screen: &mut RawScreen);
}

pub fn init_terminal(
) -> Result<Terminal<TermionBackend<AlternateScreen<MouseTerminal<RawTerminal<Stdout>>>>>> {
    let stdout = io::stdout().into_raw_mode()?;
    let stdout = MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

#[derive(Debug, Clone)]
pub enum RawScreenInput {
    // Line(StyledLine),
    Line(Line),
    // Lines(VecDeque<StyledLine>),
    Lines(Vec<Line>),
    Key(Key),
    Tick,
    WindowResize,
    Mouse(MouseEvent),
}
