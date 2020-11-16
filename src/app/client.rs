use crate::conf;
use crate::error::Result;
use crate::event::{Event, EventHandler, NextStep, QuitHandler, RuntimeEvent, RuntimeEventHandler};
use crate::protocol::Packet;
use crate::runtime::Runtime;
use crate::signal;
use crate::ui::line::RawLine;
use crate::ui::terminal::Terminal;
use crate::ui::{render, UICallback, UIEvent};
use crate::ui::widget::{Flow, CmdBar, Border, inner_area};
use crate::ui::layout::Rect;
use crate::userinput;
use crossbeam_channel::{unbounded, Sender};
use std::{io, thread};

/// 启动转发用户输入的后台线程
pub fn start_userinput_handle(evttx: Sender<Event>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        if let Err(e) = userinput::subscribe_userinput(evttx) {
            eprintln!("userinput error {}", e);
        }
    })
}

/// 启动监听窗口变化的后台线程
pub fn start_signal_handle(evttx: Sender<Event>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        if let Err(e) = signal::subscribe_signals(evttx) {
            eprintln!("signal error {}", e);
        }
    })
}

/// 启动UI渲染的后台线程
pub fn start_ui_handle(
    termconf: conf::Term,
    evttx: Sender<Event>,
) -> Result<(Sender<UIEvent>, thread::JoinHandle<()>)> {
    let (uitx, uirx) = unbounded::<UIEvent>();

    let handle = thread::spawn(move || {
        // let mut screen = RawScreen::new(termconf);
        let (width, height) = termion::terminal_size().unwrap();
        // let mut window = Window::new(width as usize, height as usize, termconf);
        let flowarea = Rect{x: 1, y: 1, width, height: height - 3};
        let mut flow = Flow::new(flowarea, 2000, true);
        let cmdborder = Border::Rounded;
        let cmdborderarea = Rect{x: 1, y: 1, width, height: 3};
        let cmdarea = inner_area(cmdborderarea, true);
        let mut cmdbar = CmdBar::new('.');
        
        let mut cb = EventBusCallback(evttx);
        let mut terminal = match Terminal::init() {
            Err(e) => {
                eprintln!("error init raw terminal {}", e);
                cb.on_quit();
                return;
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

        loop {
            match render(&mut flow, &mut cmdbar, &mut terminal, &uirx, &mut cb) {
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
    });
    Ok((uitx, handle))
}



pub fn render<C: UICallback>(
    flow: &mut Flow,
    cmdbar: &mut CmdBar,
    terminal: &mut Terminal,
    uirx: &Receiver<UIEvent>,
    cb: &mut C,
) -> Result<bool> {
    match uirx.recv()? {
        UIEvent::Key(key) => match key {
            Key::Char('\n') => {
                match cmdbar.take() {
                    CmdOut::Script(s) => {
                        cb.on_script(s);
                    }
                    CmdOut::Cmd(s) => {
                        cb.on_cmd(s);
                    }
                }
            }
            Key::Char(c) => {
                cmdbar.push_char(c);
            }
            Key::Backspace => {
                cmdbar.pop_char();
            }
            Key::Ctrl('q') => {
                cb.on_quit();
                return Ok(true);
            }
            k => {
                eprintln!("unhandled key {:?}", k);
            }
        },
        UIEvent::Lines(lines) => flow.push_lines(lines),
        UIEvent::Line(line) => flow.push_line(line),
        UIEvent::Mouse(_) => {
            // not to render the screen
            return Ok(false);
        }
        UIEvent::Tick | UIEvent::WindowResize => (),
    }
    terminal.render_widget(&mut flow, flowarea, true)?;
    terminal.render_widget(&mut , area, cjk)
    terminal.flush()?;
    Ok(false)
}

pub fn draw(evttx: Receiver<Event>) {
    // let mut screen = RawScreen::new(termconf);
    let (width, height) = termion::terminal_size().unwrap();
    // let mut window = Window::new(width as usize, height as usize, termconf);
    let flowarea = Rect{x: 1, y: 1, width, height: height - 3};
    let mut flow = Flow::new(flowarea, 2000, true);
    let cmdborder = Border::Rounded;
    let cmdborderarea = Rect{x: 1, y: 1, width, height: 3};
    let cmdarea = inner_area(cmdborderarea, true);
    let mut cmdbar = CmdBar::new('.');
    
    let mut cb = EventBusCallback(evttx);
    let mut terminal = match Terminal::init() {
        Err(e) => {
            eprintln!("error init raw terminal {}", e);
            cb.on_quit();
            return;
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

    loop {
        match render(&mut flow, &mut cmdbar, &mut terminal, &uirx, &mut cb) {
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


pub fn start_to_server_handle(mut to_server: impl io::Write + Send + 'static) -> Sender<Packet> {
    let (tx, rx) = unbounded::<Packet>();
    thread::spawn(move || loop {
        match rx.recv() {
            Err(e) => {
                eprintln!("channel receive to-server message error {}", e);
                return;
            }
            Ok(pkt) => {
                if let Err(e) = pkt.write_to(&mut to_server) {
                    eprintln!("send message to server error {}", e);
                    return;
                }
            }
        }
    });
    tx
}

pub fn start_from_server_handle(
    evttx: Sender<Event>,
    mut from_server: impl io::Read + Send + 'static,
) {
    thread::spawn(move || loop {
        match Packet::read_from(&mut from_server) {
            Err(e) => {
                eprintln!("receive server message error {}", e);
                evttx.send(Event::ServerDown).unwrap();
                return;
            }
            Ok(Packet::Lines(lines)) => {
                evttx.send(Event::LinesFromServer(lines)).unwrap();
            }
            Ok(_) => (),
        }
    });
}

pub struct Client {
    uitx: Sender<UIEvent>,
    srvtx: Sender<Packet>,
}

impl Client {
    pub fn new(uitx: Sender<UIEvent>, srvtx: Sender<Packet>) -> Self {
        Self { uitx, srvtx }
    }
}

impl EventHandler for Client {
    fn on_event(&mut self, evt: Event, rt: &mut Runtime) -> Result<NextStep> {
        match evt {
            Event::Quit => return Ok(NextStep::Quit),
            // 以下事件发送给UI线程处理
            Event::Tick => {
                // todo: implements trigger by tick
                self.uitx.send(UIEvent::Tick)?;
            }
            Event::TerminalKey(k) => {
                self.uitx.send(UIEvent::Key(k))?;
            }
            Event::TerminalMouse(m) => {
                self.uitx.send(UIEvent::Mouse(m))?;
            }
            Event::WindowResize => {
                self.uitx.send(UIEvent::WindowResize)?;
            }
            // 以下事件交给运行时处理
            Event::LinesFromServer(lines) => {
                rt.process_world_lines(lines);
            }
            Event::UserInputLine(cmd) => {
                rt.preprocess_user_cmd(cmd);
            }
            Event::UserScriptLine(s) => {
                rt.process_user_scripts(s);
            }
            Event::ServerDown => {
                eprintln!("server down or not reachable");
                // let user quit
                rt.queue
                    .push_line(RawLine::err("与服务器断开了连接，请关闭并重新连接"));
            }
            // client模式不支持客户端连接
            Event::NewClient(..)
            | Event::ClientAuthFail
            | Event::ClientAuthSuccess(_)
            | Event::ClientDisconnect
            | Event::TelnetBytes(_)
            | Event::WorldBytes(_)
            | Event::WorldLines(_)
            | Event::WorldDisconnected => {
                unreachable!("standalone mode does not support event {:?}", evt);
            }
        }
        Ok(NextStep::Run)
    }
}

impl RuntimeEventHandler for Client {
    fn on_runtime_event(&mut self, evt: RuntimeEvent, _rt: &mut Runtime) -> Result<NextStep> {
        match evt {
            RuntimeEvent::SwitchCodec(_) => unreachable!("client mode does not support {:?}", evt),
            RuntimeEvent::StringToMud(mut s) => {
                if !s.ends_with('\n') {
                    s.push('\n');
                }
                self.srvtx.send(Packet::Text(s))?;
            }
            RuntimeEvent::DisplayLines(lines) => {
                self.uitx.send(UIEvent::Lines(lines.into_vec()))?;
            }
        }
        Ok(NextStep::Run)
    }
}

pub struct QuitClient(Option<thread::JoinHandle<()>>);

impl QuitClient {
    pub fn new(handle: thread::JoinHandle<()>) -> Self {
        Self(Some(handle))
    }
}

impl QuitHandler for QuitClient {
    fn on_quit(&mut self) {
        if let Some(handle) = self.0.take() {
            handle.join().unwrap();
        }
    }
}
