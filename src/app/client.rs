use crate::conf;
use crate::error::Result;
use crate::event::{Event, EventHandler, NextStep, QuitHandler, RuntimeEvent, RuntimeEventHandler};
use crate::protocol::Packet;
use crate::runtime::Runtime;
use crate::signal;
use crate::style::StyledLine;
use crate::ui::init_terminal;
use crate::ui::{RawScreen, RawScreenCallback, RawScreenInput};
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
) -> Result<(Sender<RawScreenInput>, thread::JoinHandle<()>)> {
    let (uitx, uirx) = unbounded::<RawScreenInput>();

    let handle = thread::spawn(move || {
        let mut screen = RawScreen::new(termconf);
        let mut cb = EventBusCallback(evttx);
        let mut terminal = match init_terminal() {
            Err(e) => {
                eprintln!("error init raw terminal {}", e);
                return;
            }
            Ok(terminal) => terminal,
        };
        loop {
            match screen.render(&mut terminal, &uirx, &mut cb) {
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

pub struct EventBusCallback(Sender<Event>);

impl RawScreenCallback for EventBusCallback {
    fn on_cmd(&mut self, _screen: &mut RawScreen, cmd: String) {
        self.0.send(Event::UserInputLine(cmd)).unwrap()
    }

    fn on_script(&mut self, _screen: &mut RawScreen, script: String) {
        self.0.send(Event::UserScriptLine(script)).unwrap();
    }

    fn on_quit(&mut self, _screen: &mut RawScreen) {
        self.0.send(Event::Quit).unwrap();
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
    thread::spawn(move || {
        loop {
            match Packet::read_from(&mut from_server) {
                Err(e) => {
                    eprintln!("receive server message error {}", e);
                    evttx.send(Event::ServerDown).unwrap();
                    return;
                }
                Ok(Packet::StyledText(line, ended)) => {
                    // here the original text is already processed by server,
                    // so may be not identical to original text sent from MUD
                    let orig = line.iter().fold(String::new(), |mut s, acc| {
                        s.push_str(&acc.content);
                        s
                    });
                    evttx
                        .send(Event::StyledLineFromServer(StyledLine {
                            spans: line,
                            orig,
                            ended,
                        }))
                        .unwrap();
                }
                Ok(_) => (),
            }
        }
    });
}

pub struct Client {
    uitx: Sender<RawScreenInput>,
    srvtx: Sender<Packet>,
}

impl Client {
    pub fn new(uitx: Sender<RawScreenInput>, srvtx: Sender<Packet>) -> Self {
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
                self.uitx.send(RawScreenInput::Tick)?;
            }
            Event::TerminalKey(k) => {
                self.uitx.send(RawScreenInput::Key(k))?;
            }
            Event::TerminalMouse(m) => {
                self.uitx.send(RawScreenInput::Mouse(m))?;
            }
            Event::WindowResize => {
                self.uitx.send(RawScreenInput::WindowResize)?;
            }
            // 以下事件交给运行时处理
            Event::StyledLineFromServer(line) => {
                rt.process_mud_line(line);
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
                let err_line =
                    StyledLine::err(String::from("与服务器断开了连接，请关闭并重新连接"));
                rt.queue.push_styled_line(err_line);
            }
            // client模式不支持客户端连接
            Event::NewClient(..)
            | Event::ClientAuthFail
            | Event::ClientAuthSuccess(_)
            | Event::ClientDisconnect
            | Event::TelnetBytesToMud(_)
            | Event::BytesFromMud(_)
            | Event::StyledLinesFromMud(_) => {
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
                self.uitx.send(RawScreenInput::Lines(lines))?;
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
