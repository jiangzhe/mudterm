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

use crate::error::{Result, Error};
use crate::ui::terminal::Terminal;
use crate::event::Event;
use line::RawLine;
use termion::event::{Key, MouseEvent};
use crossbeam_channel::Receiver;
use widget::{Flow, CmdBar, CmdOut, Border};
use layout::Rect;
use crossbeam_channel::{Sender, Receiver};

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
    // cmdborder: Border,
    // cmdborderarea: Rect,
    cmdbar: CmdBar,
    cmdarea: Rect,
    terminal: Terminal,
    uirx: Receiver<UIEvent>,
    uicb: C
}

impl Screen<EventBusCallback> {

    pub fn init(evttx: Sender<Event>, uirx: Receiver<UIEvent>) -> Result<Self> {
        let (width, height) = termion::terminal_size().unwrap();
        // let mut window = Window::new(width as usize, height as usize, termconf);
        let flowarea = Rect{x: 1, y: 1, width, height: height - 3};
        let mut flow = Flow::new(flowarea, 2000, true);
        let cmdborder = Border::Rounded;
        let cmdborderarea = Rect{x: 1, y: 1, width, height: 3};
        let cmdarea = widget::inner_area(cmdborderarea, true);
        let mut cmdbar = CmdBar::new('.');
        
        let mut uicb = EventBusCallback(evttx);
        let mut terminal = match Terminal::init() {
            Err(e) => {
                eprintln!("error init raw terminal {}", e);
                uicb.on_quit();
                return Err(Error::RuntimeError("screen initialization failed".to_owned()));
            }
            Ok(terminal) => {
                eprintln!("raw terminal intiailized");
                terminal
            }
        };
        terminal.render_widget(&mut flow, flowarea, true).unwrap();
        // terminal.flush(area)?;
        terminal.render_widget(&mut cmdborder, cmdborderarea, true).unwrap();
        // terminal.flush(area)?;
        terminal.render_widget(&mut cmdbar, cmdarea, true).unwrap();
        terminal.flush(vec![flowarea, cmdborderarea, cmdarea]).unwrap();

        Ok(Self{
            flow,
            flowarea,
            // cmdborder,
            cmdbar,
            cmdarea,
            terminal,
            uirx,
            uicb,
        })
    }

    fn event_loop(&mut self) {
        loop {
            match self.handle_event() {
                Err(e) => {
                    eprintln!("error render raw terminal {}", e);
                    return;
                }
                Ok(true) => {
                    eprintln!("exiting raw terminal");
                    return;
                }
                _ => (),
            }
        }
    }

    fn handle_event(&mut self) -> Result<bool> {
        match self.uirx.recv()? {
            UIEvent::Key(key) => match key {
                Key::Char('\n') => {
                    match self.cmdbar.take() {
                        CmdOut::Script(s) => {
                            self.uicb.on_script(s);
                        }
                        CmdOut::Cmd(s) => {
                            self.uicb.on_cmd(s);
                        }
                    }
                }
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
        self.terminal.render_widget(&mut self.flow, self.flowarea, true)?;
        self.terminal.render_widget(&mut self.cmdbar, self.cmdarea, true)?;
        self.terminal.flush(vec![self.flowarea, self.cmdarea])?;
        Ok(false)
    }
}
