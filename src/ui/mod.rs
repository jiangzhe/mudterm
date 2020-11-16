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
// pub mod window;

use crate::error::{Error, Result};
use crate::event::Event;
use crate::ui::terminal::Terminal;
use crossbeam_channel::Sender;
use layout::Rect;
use line::RawLine;
use termion::event::{Key, MouseEvent};
use widget::{Block, CmdBar, CmdOut, Flow, Widget};

pub trait UICallback {
    fn on_cmd(&mut self, cmd: String);

    fn on_script(&mut self, script: String);

    fn on_quit(&mut self);
}

pub struct EventBusCallback(Sender<Event>);

impl UICallback for EventBusCallback {
    fn on_cmd(&mut self, cmd: String) {
        self.0.send(Event::UserInputLine(cmd)).unwrap()
    }

    fn on_script(&mut self, script: String) {
        self.0.send(Event::UserScriptLine(script)).unwrap();
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
        let cmdbar = CmdBar::new('.', true);
        let mut uicb = EventBusCallback(evttx);
        let terminal = match Terminal::init() {
            Err(e) => {
                eprintln!("error init raw terminal {}", e);
                uicb.on_quit();
                return Err(Error::RuntimeError(
                    "screen initialization failed".to_owned(),
                ));
            }
            Ok(terminal) => {
                eprintln!("raw terminal intiailized");
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
                Key::Char('\n') => match self.cmdbar.take() {
                    CmdOut::Script(s) => {
                        self.uicb.on_script(s);
                    }
                    CmdOut::Cmd(s) => {
                        self.uicb.on_cmd(s);
                    }
                },
                Key::Char(c) => {
                    self.cmdbar.push_char(c);
                }
                Key::Backspace => {
                    self.cmdbar.pop_char();
                }
                Key::Ctrl('q') => {
                    self.uicb.on_quit();
                    return Ok(true);
                }
                k => {
                    eprintln!("unhandled key {:?}", k);
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
        // eprintln!("cursur ({}, {})", cursor_x, cursor_y);
        // eprintln!("cmdarea={:?}", self.cmdarea);
        self.terminal.set_cursor(cursor_x, cursor_y)?;
        Ok(false)
    }

    pub fn flush(&mut self) -> Result<()> {
        self.terminal
            .render_widget(&mut self.flow, self.flowarea)?;
        self.terminal
            .render_widget(&mut self.cmdbar, self.cmdarea)?;
        self.terminal.flush(vec![self.flowarea, self.cmdarea])?;
        Ok(())
    }

    /// 代理Widget更新
    pub fn render_widget<W: Widget>(
        &mut self,
        widget: &mut W,
        area: Rect,
        cjk: bool,
    ) -> Result<()> {
        self.terminal.render_widget(widget, area)
    }
}
