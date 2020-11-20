use crate::auth;
use crate::error::Result;
use crate::event::{Event, EventHandler, NextStep, QuitHandler};
use crate::protocol::Packet;
use crate::runtime::{Runtime, RuntimeOutput, RuntimeOutputHandler};
use crate::transport::{Outbound, Telnet, TelnetEvent};
use crate::ui::line::RawLines;
use crate::ui::UserOutput;
use crossbeam_channel::{unbounded, Sender};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::Duration;
use std::{io, thread};

/// 启动线程接收MUD消息
pub fn start_from_mud_handle(evttx: Sender<Event>, from_mud: impl io::Read + Send + 'static) {
    thread::spawn(move || {
        let mut telnet = Telnet::new(from_mud, 4096);
        loop {
            match telnet.recv() {
                Err(e) => {
                    log::error!("channel receive telnet message error {}", e);
                    return;
                }
                Ok(TelnetEvent::Disconnected) => {
                    // once disconnected, stop the thread
                    let _ = evttx.send(Event::WorldDisconnected);
                    break;
                }
                Ok(TelnetEvent::Empty) => (),
                Ok(TelnetEvent::DataToSend(bs)) => evttx.send(Event::TelnetBytes(bs)).unwrap(),
                Ok(TelnetEvent::Text(bs)) => evttx.send(Event::WorldBytes(bs)).unwrap(),
            }
        }
    });
}

/// 启动线程发送MUD消息
pub fn start_to_mud_handle(
    evttx: Sender<Event>,
    to_mud: impl io::Write + Send + 'static,
) -> Sender<Vec<u8>> {
    let (tx, rx) = unbounded::<Vec<u8>>();
    thread::spawn(move || {
        let mut outbound = Outbound::new(to_mud);
        loop {
            match rx.recv() {
                Err(e) => {
                    log::error!("channel receive outbound message error {}", e);
                    let _ = evttx.send(Event::WorldDisconnected);
                    return;
                }
                Ok(bs) => {
                    if let Err(e) = outbound.send(bs) {
                        log::error!("send server error: {}", e);
                    }
                }
            }
        }
    });
    tx
}

/// 启动线程监听本地端口
pub fn start_server_listener_handle(listener: TcpListener, evttx: Sender<Event>) {
    thread::spawn(move || loop {
        let (conn, addr) = match listener.accept() {
            Err(e) => {
                log::error!("accept new client connection error {}", e);
                thread::sleep(Duration::from_secs(1));
                continue;
            }
            Ok((conn, addr)) => (conn, addr),
        };
        evttx.send(Event::NewClient(conn, addr)).unwrap();
    });
}

/// 启动线程向客户端发送消息
fn start_to_client_handle(conn: TcpStream, pass: String, evttx: Sender<Event>) -> Sender<Packet> {
    let (tx, rx) = unbounded::<Packet>();
    thread::spawn(move || {
        // do authentication first
        let mut conn = match auth::server_auth(conn, &pass) {
            Err(_) => {
                let _ = evttx.send(Event::ClientAuthFail);
                return;
            }
            Ok(conn) => conn,
        };
        let conn_recv = conn.try_clone().unwrap();
        evttx.send(Event::ClientAuthSuccess(conn_recv)).unwrap();
        // proxy messages to client
        loop {
            match rx.recv() {
                Err(e) => {
                    log::error!(
                        "channel receive server message error {}, stop this thread",
                        e
                    );
                    return;
                }
                Ok(sm) => {
                    let pkt: Packet = sm.into();
                    if pkt.write_to(&mut conn).is_err() {
                        let _ = evttx.send(Event::ClientDisconnect);
                        break;
                    }
                }
            }
        }
        log::info!("to_client handle exited");
    });
    tx
}

/// 启动线程从客户端接收消息
fn start_from_client_handle(mut conn: TcpStream, evttx: Sender<Event>) {
    thread::spawn(move || loop {
        match Packet::read_from(&mut conn) {
            Err(_) => {
                let _ = evttx.send(Event::ClientDisconnect);
                break;
            }
            Ok(Packet::Text(s)) => {
                evttx.send(Event::UserOutput(UserOutput::Cmd(s))).unwrap();
            }
            Ok(other) => {
                log::warn!("received unexpected packet from client {:?}", other);
            }
        }
    });
}

/// server app
pub struct Server {
    worldtx: Sender<Vec<u8>>,
    pass: String,
    buffer: RawLines,
    to_cli: Option<(Sender<Packet>, SocketAddr)>,
}

impl Server {
    pub fn new(worldtx: Sender<Vec<u8>>, pass: String, init_max_lines: usize) -> Self {
        let buffer = RawLines::with_capacity(init_max_lines);
        Self {
            worldtx,
            pass,
            buffer,
            to_cli: None,
        }
    }
}

impl EventHandler for Server {
    fn on_event(&mut self, evt: Event, rt: &mut Runtime) -> Result<NextStep> {
        match evt {
            Event::NewClient(conn, addr) => {
                log::info!("client connected from {:?}", addr);
                if let Some((_, ref addr)) = self.to_cli {
                    log::warn!(
                        "drop the connection because only one client allowed, current client {:?}",
                        addr
                    );
                    return Ok(NextStep::Run);
                }
                let tx = start_to_client_handle(conn, self.pass.clone(), rt.evttx.clone());
                self.to_cli = Some((tx, addr));
            }
            Event::ClientAuthFail => {
                log::info!("client auth failed");
                self.to_cli.take();
            }
            Event::ClientAuthSuccess(mut conn) => {
                log::info!("client auth succeeded, starting thread to handle incoming messages");
                let lines = self.buffer.to_vec();
                // todo: separate multiple batch
                let pkt = Packet::Lines(lines);
                if let Err(e) = pkt.write_to(&mut conn) {
                    log::error!("channel send client style text error {}", e);
                    // maybe client disconnected, discard this connection
                    return Ok(NextStep::Run);
                }
                start_from_client_handle(conn, rt.evttx.clone());
            }
            Event::ClientDisconnect => {
                log::info!("client disconnected");
                self.to_cli.take();
            }
            // 直接发送给MUD
            Event::TelnetBytes(bs) => {
                self.worldtx.send(bs)?;
            }
            // 以下事件交给运行时处理
            Event::WorldBytes(bs) => {
                rt.process_bytes_from_mud(&bs)?;
            }
            Event::WorldLines(lines) => {
                rt.process_world_lines(lines);
            }
            Event::UserOutput(output) => {
                rt.process_user_output(output);
            }
            Event::Tick => {
                // todo: implements trigger by tick
            }
            Event::WorldDisconnected => {
                log::warn!("world down or disconnected, shutdown server");
                return Ok(NextStep::Quit);
            }
            Event::Quit
            | Event::LinesFromServer(_)
            | Event::TerminalKey(_)
            | Event::TerminalMouse(_)
            | Event::WindowResize
            | Event::ServerDown => unreachable!("standalone mode does not support event {:?}", evt),
        }
        Ok(NextStep::Run)
    }
}

impl RuntimeOutputHandler for Server {
    fn on_runtime_output(&mut self, output: RuntimeOutput, rt: &mut Runtime) -> Result<NextStep> {
        match output {
            RuntimeOutput::ToServer(mut s) => {
                if !s.ends_with('\n') {
                    s.push('\n');
                }
                let bs = rt.encode(&s)?;
                self.worldtx.send(bs)?;
            }
            RuntimeOutput::ToUI(raw, _) => {
                if let Some((clitx, _)) = self.to_cli.as_mut() {
                    let lines = raw.into_vec();
                    if let Err(e) = clitx.send(Packet::Lines(lines)) {
                        log::error!("channel send to-client message error {}", e);
                    }
                }
            }
        }
        Ok(NextStep::Run)
    }
}

pub struct QuitServer;

impl QuitHandler for QuitServer {
    fn on_quit(&mut self) {}
}
