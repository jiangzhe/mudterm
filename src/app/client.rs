use crate::error::Result;
use crate::event::{Event, EventHandler, NextStep, QuitHandler};
use crate::proto::cli::Packet;
use crate::runtime::{Engine, EngineAction, RuntimeOutput, RuntimeOutputHandler};
use crate::signal;
use crate::ui::line::Lines;
use crate::ui::{Screen, UIEvent};
use crate::userinput;
use crossbeam_channel::{unbounded, Sender};
use std::{io, thread};

/// 启动转发用户输入的后台线程
pub fn start_userinput_handle(evttx: Sender<Event>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        if let Err(e) = userinput::subscribe_userinput(evttx) {
            log::error!("userinput error {}", e);
        }
    })
}

/// 启动监听窗口变化的后台线程
pub fn start_signal_handle(evttx: Sender<Event>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        if let Err(e) = signal::subscribe_signals(evttx) {
            log::error!("signal error {}", e);
        }
    })
}

/// 启动UI渲染的后台线程
pub fn start_ui_handle(
    evttx: Sender<Event>,
) -> Result<(Sender<UIEvent>, thread::JoinHandle<()>)> {
    let (uitx, uirx) = unbounded::<UIEvent>();
    let handle = thread::spawn(move || {
        let mut screen = match Screen::init(evttx.clone()) {
            Ok(screen) => screen,
            Err(e) => {
                log::error!("failed to initialize screen {}", e);
                let _ = evttx.send(Event::Quit);
                return;
            }
        };

        loop {
            match uirx.recv() {
                Err(e) => {
                    log::error!("channel receive ui event error {}", e);
                    let _ = evttx.send(Event::Quit);
                    break;
                }
                Ok(evt) => {
                    log::trace!("processing UIEvent {:?}", evt);
                    let to_quit = match screen.process_event(evt) {
                        Ok(f) => f,
                        Err(e) => {
                            log::error!("failed to process event {}", e);
                            let _ = evttx.send(Event::Quit);
                            break;
                        }
                    };
                    if to_quit {
                        break;
                    }
                }
            }
        }
    });
    Ok((uitx, handle))
}

pub fn start_to_server_handle(mut to_server: impl io::Write + Send + 'static) -> Sender<Packet> {
    let (tx, rx) = unbounded::<Packet>();
    thread::spawn(move || loop {
        match rx.recv() {
            Err(e) => {
                log::error!("channel receive to-server message error {}", e);
                return;
            }
            Ok(pkt) => {
                log::trace!("processing packet {:?}", pkt);
                if let Err(e) = pkt.write_to(&mut to_server) {
                    log::error!("send message to server error {}", e);
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
                log::error!("receive server message error {}", e);
                evttx.send(Event::ServerDown).unwrap();
                return;
            }
            Ok(Packet::Lines(lines)) => {
                log::trace!("receiving server message {:?}", lines);
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
    fn on_event(&mut self, evt: Event, engine: &mut Engine) -> Result<NextStep> {
        match evt {
            Event::Quit => return Ok(NextStep::Quit),
            // 以下事件发送给UI线程处理
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
                engine.push(EngineAction::ProcessWorldLines(lines));
            }
            Event::UserOutput(output) => {
                engine.push(EngineAction::ExecuteUserOutput(output));
            }
            Event::Timer(timer) => {
                engine.push(EngineAction::ExecuteTimer(timer));
            }
            Event::ServerDown => {
                log::error!("server down or not reachable");
                // let user quit
                let err_lines = Lines::fmt_err("与服务器断开了连接，请关闭并重新连接");
                for err_line in err_lines.into_vec() {
                    engine.push(EngineAction::SendLineToUI(err_line, None));
                }
            }
            // client模式不支持客户端连接
            Event::NewClient(..)
            | Event::ClientAuthFail
            | Event::ClientAuthSuccess(_)
            | Event::ClientDisconnect
            | Event::TelnetBytes(_)
            | Event::WorldBytes(_)
            | Event::WorldDisconnected => {
                unreachable!("standalone mode does not support event {:?}", evt);
            }
        }
        Ok(NextStep::Run)
    }
}

impl RuntimeOutputHandler for Client {
    fn on_runtime_output(&mut self, output: RuntimeOutput) -> Result<NextStep> {
        match output {
            RuntimeOutput::ToServer(bs) => {
                self.srvtx
                    .send(Packet::Text(String::from_utf8(bs).unwrap()))?;
            }
            RuntimeOutput::ToUI(_, styled) => {
                self.uitx.send(UIEvent::Lines(styled))?;
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
