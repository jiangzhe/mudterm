use crate::auth;
use crate::error::Result;
use crate::event::{Event, EventHandler, NextStep, QuitHandler, RuntimeEvent, RuntimeEventHandler};
use crate::protocol::Packet;
use crate::runtime::Runtime;
use crate::transport::{Outbound, Telnet, TelnetEvent};
use crate::ui::line::RawLines;
use crossbeam_channel::{unbounded, Sender};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::Duration;
use std::{io, thread};

/// 启动线程接收MUD消息
pub fn start_from_mud_handle(evttx: Sender<Event>, from_mud: impl io::Read + Send + 'static) {
    thread::spawn(move || {
        let mut inbound = Telnet::new(from_mud, 4096);
        loop {
            match inbound.recv() {
                Err(e) => {
                    eprintln!("channel receive inbound message error {}", e);
                    return;
                }
                Ok(TelnetEvent::Disconnected) => {
                    // once disconnected, stop the thread
                    break;
                }
                Ok(TelnetEvent::Empty) => (),
                Ok(TelnetEvent::DataToSend(bs)) => evttx.send(Event::TelnetBytesToMud(bs)).unwrap(),
                Ok(TelnetEvent::Text(bs)) => evttx.send(Event::BytesFromMud(bs)).unwrap(),
            }
        }
    });
}

/// 启动线程发送MUD消息
pub fn start_to_mud_handle(to_mud: impl io::Write + Send + 'static) -> Sender<Vec<u8>> {
    let (tx, rx) = unbounded::<Vec<u8>>();
    thread::spawn(move || {
        let mut outbound = Outbound::new(to_mud);
        loop {
            match rx.recv() {
                Err(e) => {
                    eprintln!("{}", e);
                    return;
                }
                Ok(bs) => {
                    if let Err(e) = outbound.send(bs) {
                        eprintln!("send server error: {}", e);
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
                eprintln!("accept new client connection error {}", e);
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
                evttx.send(Event::ClientAuthFail).unwrap();
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
                    eprintln!(
                        "channel receive server message error {}, stop this thread",
                        e
                    );
                    return;
                }
                Ok(sm) => {
                    let pkt: Packet = sm.into();
                    if pkt.write_to(&mut conn).is_err() {
                        evttx.send(Event::ClientDisconnect).unwrap();
                        break;
                    }
                }
            }
        }
        eprintln!("to_client handle exited");
    });
    tx
}

/// 启动线程从客户端接收消息
fn start_from_client_handle(mut conn: TcpStream, evttx: Sender<Event>) {
    thread::spawn(move || loop {
        match Packet::read_from(&mut conn) {
            Err(_) => {
                evttx.send(Event::ClientDisconnect).unwrap();
                break;
            }
            Ok(Packet::Text(s)) => {
                evttx.send(Event::UserInputLine(s)).unwrap();
            }
            Ok(other) => {
                eprintln!("received unexpected packet from client {:?}", other);
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
            Event::Quit => return Ok(NextStep::Quit),
            Event::NewClient(conn, addr) => {
                eprintln!("client connected from {:?}", addr);
                if let Some((_, ref addr)) = self.to_cli {
                    eprintln!(
                        "drop the connection because only one client allowed, current client {:?}",
                        addr
                    );
                    return Ok(NextStep::Run);
                }
                let tx = start_to_client_handle(conn, self.pass.clone(), rt.evttx.clone());
                self.to_cli = Some((tx, addr));
            }
            Event::ClientAuthFail => {
                eprintln!("client auth failed");
                self.to_cli.take();
            }
            Event::ClientAuthSuccess(mut conn) => {
                eprintln!("client auth succeeded, starting thread to handle incoming messages");
                // let lastn = self.line_cache.lastn(50000);
                let lines = self.buffer.to_vec();
                // todo: separate multiple batch
                let pkt = Packet::Lines(lines);
                if let Err(e) = pkt.write_to(&mut conn) {
                    eprintln!("channel send client style text error {}", e);
                    // maybe client disconnected, discard this connection
                    return Ok(NextStep::Run);
                }
                start_from_client_handle(conn, rt.evttx.clone());
            }
            Event::ClientDisconnect => {
                eprintln!("client disconnected");
                self.to_cli.take();
            }
            // 直接发送给MUD
            Event::TelnetBytesToMud(bs) => {
                self.worldtx.send(bs)?;
            }
            // 以下事件交给运行时处理
            Event::BytesFromMud(bs) => {
                rt.process_bytes_from_mud(&bs)?;
            }
            Event::LinesFromMud(lines) => {
                rt.process_mud_lines(lines);
            }
            Event::UserInputLine(cmd) => {
                rt.preprocess_user_cmd(cmd);
            }
            Event::Tick => {
                // todo: implements trigger by tick
            }
            Event::LinesFromServer(_)
            | Event::TerminalKey(_)
            | Event::TerminalMouse(_)
            | Event::WindowResize
            | Event::UserScriptLine(_)
            | Event::ServerDown => unreachable!("standalone mode does not support event {:?}", evt),
        }
        Ok(NextStep::Run)
    }
}

impl RuntimeEventHandler for Server {
    fn on_runtime_event(&mut self, evt: RuntimeEvent, rt: &mut Runtime) -> Result<NextStep> {
        match evt {
            RuntimeEvent::SwitchCodec(code) => {
                rt.mud_codec.switch_codec(code);
            }
            RuntimeEvent::StringToMud(mut s) => {
                if !s.ends_with('\n') {
                    s.push('\n');
                }
                let bs = rt.mud_codec.encode(&s)?;
                self.worldtx.send(bs)?;
            }
            RuntimeEvent::DisplayLines(lines) => {
                if let Some((clitx, _)) = self.to_cli.as_mut() {
                    let lines = lines.into_vec();
                    if let Err(e) = clitx.send(Packet::Lines(lines)) {
                        eprintln!("channel send to-client message error {}", e);
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
