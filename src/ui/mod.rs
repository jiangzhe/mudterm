// pub mod flow;
pub mod ansi;
pub mod buffer;
pub mod layout;
pub mod line;
pub mod span;
pub mod style;
pub mod symbol;
pub mod terminal;
pub mod widget;
pub mod width;

use crate::error::{Error, Result};
use crate::event::Event;
use crate::ui::terminal::Terminal;
use crossbeam_channel::Sender;
use layout::Rect;
use line::RawLine;
use termion::event::{Key, MouseEvent};
use widget::{CmdBar, Flow, Widget};

#[derive(Debug, Clone, PartialEq)]
pub enum UserOutput {
    Cmd(String),
    Script(String),
}

impl Default for UserOutput {
    fn default() -> Self {
        Self::Cmd(String::new())
    }
}

impl AsRef<str> for UserOutput {
    fn as_ref(&self) -> &str {
        match self {
            Self::Cmd(s) => s,
            Self::Script(s) => s,
        }
    }
}

impl UserOutput {
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
        *self = UserOutput::default();
    }
}

pub trait UICallback {
    fn on_output(&mut self, output: UserOutput);

    fn on_quit(&mut self);
}

pub struct EventBusCallback(Sender<Event>);

impl UICallback for EventBusCallback {
    fn on_output(&mut self, output: UserOutput) {
        self.0.send(Event::UserOutput(output)).unwrap()
    }

    fn on_quit(&mut self) {
        self.0.send(Event::Quit).unwrap();
    }
}

#[derive(Debug, Clone)]
pub enum UIEvent {
    Line(RawLine),
    Lines(Vec<RawLine>),
    Key(Key),
    Tick,
    WindowResize,
    Mouse(MouseEvent),
}

pub struct Screen<C> {
    flow: Flow,
    flowarea: Rect,
    cmdbar: CmdBar,
    cmdarea: Rect,
    terminal: Terminal,
    uicb: C,
}

impl Screen<EventBusCallback> {
    pub fn init(evttx: Sender<Event>) -> Result<Self> {
        let (width, height) = termion::terminal_size()?;
        // 流占据主屏幕大半部分
        let flowarea = Rect {
            x: 1,
            y: 1,
            width,
            height: height - 3,
        };
        let flow = Flow::new(flowarea, 2000, true);
        // 命令行占据屏幕最下部3行
        let cmdarea = Rect {
            x: 1,
            y: height - 2,
            width,
            height: 3,
        };
        let cmdbar = CmdBar::new('.', true, 200);
        let mut uicb = EventBusCallback(evttx);
        let terminal = match Terminal::init() {
            Err(e) => {
                log::error!("error init raw terminal {}", e);
                uicb.on_quit();
                return Err(Error::RuntimeError(
                    "screen initialization failed".to_owned(),
                ));
            }
            Ok(terminal) => {
                log::debug!("raw terminal intiailized");
                terminal
            }
        };

        let mut screen = Self {
            flow,
            flowarea,
            cmdbar,
            cmdarea,
            terminal,
            uicb,
        };
        screen.flush()?;
        Ok(screen)
    }

    pub fn process_event(&mut self, event: UIEvent) -> Result<bool> {
        match event {
            UIEvent::Key(key) => match key {
                Key::Char('\n') => self.uicb.on_output(self.cmdbar.take()),
                Key::Char(c) => {
                    self.cmdbar.push_char(c);
                }
                Key::Backspace => {
                    self.cmdbar.pop_char();
                }
                Key::Up => {
                    self.cmdbar.prev_cmd();
                }
                Key::Down => {
                    self.cmdbar.next_cmd();
                }
                Key::Ctrl('q') => {
                    self.uicb.on_quit();
                    return Ok(true);
                }
                k => {
                    log::debug!("unhandled key {:?}", k);
                }
            },
            UIEvent::Lines(lines) => self.flow.push_lines(lines),
            UIEvent::Line(line) => self.flow.push_line(line),
            UIEvent::Mouse(_) => {
                // not to render the screen
                return Ok(false);
            }
            UIEvent::Tick | UIEvent::WindowResize => (),
        }
        self.flush()?;
        let (cursor_x, cursor_y) = self.cmdbar.cursor_pos(self.cmdarea, true);
        self.terminal.set_cursor(cursor_x, cursor_y)?;
        Ok(false)
    }

    pub fn flush(&mut self) -> Result<()> {
        self.terminal.render_widget(&mut self.flow, self.flowarea)?;
        self.terminal
            .render_widget(&mut self.cmdbar, self.cmdarea)?;
        self.terminal.flush(vec![self.flowarea, self.cmdarea])?;
        Ok(())
    }

    /// 代理Widget更新
    pub fn render_widget<W: Widget>(&mut self, widget: &mut W, area: Rect) -> Result<()> {
        self.terminal.render_widget(widget, area)
    }
}
