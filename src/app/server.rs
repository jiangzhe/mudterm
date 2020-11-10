use crate::auth;
use crate::codec::{AnsiBuffer, Codec, Decoder, Encoder};
use crate::conf;
use crate::error::{Error, Result};
use crate::event::{DerivedEvent, Event};
use crate::protocol::Packet;
use crate::runtime::script::Script;
use crate::runtime::trigger::Trigger;
use crate::style::{StyleReflector, StyledLine};
use crate::transport::{Inbound, InboundMessage, Outbound};
use crate::ui::Lines;
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use std::{io, thread};

/// server mode, no UI rendering, but listen and
/// transfer UI data
pub struct Server {
    config: conf::Config,
    evtrx: Receiver<Event>,
    evttx: Sender<Event>,
    to_mud: Option<Sender<Vec<u8>>>,
    server_listener: Option<thread::JoinHandle<()>>,
    to_client: Option<Sender<Packet>>,
    gcodec: Codec,
    decoder: Decoder,
    encoder: Encoder,
    reflector: StyleReflector,
    script: Script,
    triggers: Vec<Trigger>,
    evtq: Arc<Mutex<VecDeque<DerivedEvent>>>,
    lines: Lines,
    ansi_buf: AnsiBuffer,
}

impl Server {
    pub fn with_config(config: conf::Config) -> Self {
        // event channel, processed by main loop
        let (evttx, evtrx) = unbounded();
        // server channel, processed by server thread (reader and writer)
        // let (srvtx, srvrx) = unbounded();
        // world channel, processed by world thread
        // let (worldtx, worldrx) = unbounded();
        let mut lines = Lines::new();
        lines.set_max_lines(config.term.max_lines);
        Self {
            config,
            evtrx,
            evttx,
            to_mud: None,
            server_listener: None,
            to_client: None,
            gcodec: Codec::default(),
            decoder: Decoder::default(),
            encoder: Encoder::default(),
            reflector: StyleReflector::default(),
            script: Script::new(),
            triggers: Vec::new(),
            evtq: Arc::new(Mutex::new(VecDeque::new())),
            lines,
            ansi_buf: AnsiBuffer::new(),
        }
    }

    pub fn switch_codec(&mut self, code: Codec) {
        self.gcodec = code;
        self.decoder.switch_codec(code);
        self.encoder.switch_codec(code);
    }

    pub fn start_from_mud_handle(&mut self, from_mud: std::net::TcpStream) {
        let evttx = self.evttx.clone();
        thread::spawn(move || {
            let mut inbound = Inbound::new(from_mud, 4096);
            loop {
                match inbound.recv() {
                    Err(e) => {
                        eprintln!(
                            "channel receive inbound message error {}, quick the thread",
                            e
                        );
                        return;
                    }
                    Ok(InboundMessage::Disconnected) => {
                        // once disconnected, stop the thread
                        break;
                    }
                    Ok(InboundMessage::Empty) => (),
                    Ok(InboundMessage::TelnetDataToSend(bs)) => {
                        evttx.send(Event::TelnetBytesToMud(bs)).unwrap();
                    }
                    Ok(InboundMessage::Text(bs)) => {
                        evttx.send(Event::BytesFromMud(bs)).unwrap();
                    }
                }
            }
        });
    }

    pub fn start_to_mud_handle(&mut self, to_mud: impl io::Write + Send + 'static) -> Result<()> {
        if self.to_mud.is_some() {
            return Err(Error::RuntimeError(
                "already have connection to mud".to_owned(),
            ));
        }
        let (tx, rx) = unbounded();
        thread::spawn(move || {
            let mut outbound = Outbound::new(to_mud);
            loop {
                match rx.recv() {
                    Err(e) => {
                        eprintln!(
                            "channel receive to_mud message error {}, quit the thread",
                            e
                        );
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
        self.to_mud = Some(tx);
        Ok(())
    }

    pub fn start_server_listener_handle(&mut self, listener: TcpListener) {
        debug_assert!(self.server_listener.is_none());
        let server_listener_handle = {
            let evttx = self.evttx.clone();
            thread::spawn(move || loop {
                let conn = match listener.accept() {
                    Err(e) => {
                        eprintln!("accept new client connection error {}", e);
                        thread::sleep(Duration::from_secs(1));
                        continue;
                    }
                    Ok((conn, addr)) => {
                        eprintln!("accept new client connection from {:?}", addr);
                        conn
                    }
                };
                evttx.send(Event::NewClient(conn)).unwrap();
            })
        };
        self.server_listener = Some(server_listener_handle);
    }

    fn start_to_client_handle(&mut self, conn: TcpStream) {
        if self.to_client.is_some() {
            // drop the connection
            return;
        }
        let evttx = self.evttx.clone();
        let pass = self.config.server.pass.to_owned();
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
        self.to_client = Some(tx);
    }

    fn start_from_client_handle(&mut self, mut conn: TcpStream) {
        let evttx = self.evttx.clone();
        thread::spawn(move || loop {
            match Packet::read_from(&mut conn) {
                Err(_) => {
                    evttx.send(Event::ClientDisconnect).unwrap();
                    break;
                }
                Ok(Packet::Text(s)) => {
                    evttx.send(Event::UserInputLine(s)).unwrap();
                }
                Ok(_) => (),
            }
        });
    }

    pub fn main_loop(mut self) -> Result<()> {
        let mut serverlog = File::create(&self.config.server.log_file)?;
        let vars = Arc::new(RwLock::new(HashMap::new()));
        self.script
            .setup_script_functions(vars.clone(), self.evtq.clone())?;
        loop {
            let evt = self.evtrx.recv()?;
            if self.handle_event(evt, &mut serverlog) {
                break;
            }
            // script generated events are stored in evtq
            let events: Vec<DerivedEvent> = self.evtq.lock().unwrap().drain(..).collect();
            for evt in events {
                if self.handle_derived_event(evt) {
                    break;
                }
            }
        }
        Ok(())
    }

    fn handle_event(&mut self, evt: Event, logger: &mut File) -> bool {
        match evt {
            Event::Tick => unimplemented!("tick event"),
            Event::WindowResize => unreachable!("server mode does not support WindowResize"),
            Event::UserInputLines(cmds) => unimplemented!(),
            Event::TerminalKey(_) | Event::TerminalMouse(_) => {
                unreachable!("server mode does not support terminal event")
            }
            Event::TelnetBytesToMud(bs) => {
                if let Some(ref to_mud) = self.to_mud {
                    if let Err(e) = to_mud.send(bs) {
                        eprintln!("channel send telnet bytes error {}", e);
                    }
                }
            }
            Event::Quit => return true,
            Event::NewClient(conn) => {
                self.start_to_client_handle(conn);
            }
            Event::ClientAuthFail | Event::ClientDisconnect => {
                self.to_client.take();
            }
            Event::ClientAuthSuccess(conn) => {
                // on intial connection, we need to send the recent lines to client to display
                if let Some(ref to_client) = self.to_client {
                    let init_max_lines = self.config.server.client_init_max_lines;
                    let lastn = self.lines.lastn(init_max_lines);
                    for line in lastn {
                        let sl = Packet::StyledText(line.0, true);
                        if let Err(e) = to_client.send(sl) {
                            eprintln!("channel send client style text error {}", e);
                        }
                    }
                }
                self.start_from_client_handle(conn);
            }
            Event::BytesFromMud(bs) => {
                let bs = self.ansi_buf.process(bs);
                let mut s = String::new();
                let _ = self.decoder.decode_raw_to(&bs, &mut s);
                // log server output with ansi sequence
                if self.config.server.log_ansi {
                    if let Err(e) = logger.write_all(s.as_bytes()) {
                        eprintln!("write log error {}", e);
                    }
                }
                let sms = self.reflector.reflect(s);
                // log server output without ansi sequence
                if !self.config.server.log_ansi {
                    for sm in &sms {
                        if let Err(e) = logger.write_all(sm.orig.as_bytes()) {
                            eprintln!("write log error {}", e);
                        }
                        if sm.ended {
                            if let Err(e) = logger.write_all(b"\n") {
                                eprintln!("write log error {}", e);
                            }
                        }
                    }
                }
                // send to global event bus
                self.evttx.send(Event::StyledLinesFromMud(sms)).unwrap();
            }
            Event::StyledLinesFromMud(sms) => {
                for sm in sms {
                    let text = sm.orig.clone();
                    self.push_evtq_styled_line(sm);
                    // invoke trigger for each line
                    for tr in &self.triggers {
                        // invoke at most one matched trigger
                        if tr.is_match(&text) {
                            eprintln!("trigger matched: {}", text);
                            // todo: diff actions based on target
                            if let Err(e) = self.script.exec(&tr.model.scripts) {
                                eprintln!("exec script error {}", e);
                                // also send to event bus
                                self.push_evtq_styled_line(StyledLine::err(e.to_string()));
                            }
                            break;
                        }
                    }
                }
            }
            Event::UserInputLine(mut cmd) => {
                // prepare to send to world
                if !cmd.ends_with('\n') {
                    cmd.push('\n');
                }
                let mut bs = Vec::new();
                if let Err(e) = self.encoder.encode_to(&cmd, &mut bs) {
                    eprintln!("encode to bytes error {}", e);
                }
                if let Some(ref to_mud) = self.to_mud {
                    if let Err(e) = to_mud.send(bs) {
                        eprintln!("send world input error {}", e);
                    }
                }
            }
            Event::UserScriptLine(s) => {
                if let Err(e) = self.script.exec(&s) {
                    self.push_evtq_styled_line(StyledLine::err(e.to_string()));
                }
            }
        }
        false
    }

    fn handle_derived_event(&mut self, evt: DerivedEvent) -> bool {
        match evt {
            DerivedEvent::SwitchCodec(code) => {
                self.switch_codec(code);
            }
            DerivedEvent::StringToMud(mut s) => {
                if !s.ends_with('\n') {
                    s.push('\n');
                }
                let mut bs = Vec::new();
                if let Err(e) = self.encoder.encode_to(&s, &mut bs) {
                    eprintln!("encode to bytes error {}", e);
                }
                if let Some(ref to_mud) = self.to_mud {
                    if let Err(e) = to_mud.send(bs) {
                        eprintln!("send worldtx error {}", e);
                    }
                }
            }
            DerivedEvent::DisplayLines(sms) => {
                // alway keep local storage, and send to client if connected
                if let Some(ref to_client) = self.to_client {
                    for sm in &sms {
                        let stext = Packet::StyledText(sm.spans.clone(), sm.ended);
                        if let Err(e) = to_client.send(stext) {
                            eprintln!("channel send style text error {}", e);
                        }
                    }
                }
                self.lines.push_lines(sms);
            }
        }
        false
    }

    fn push_evtq_styled_line(&self, line: StyledLine) {
        let mut evtq = self.evtq.lock().unwrap();
        if let Some(DerivedEvent::DisplayLines(lines)) = evtq.back_mut() {
            lines.push_back(line);
            return;
        }
        let mut lines = VecDeque::new();
        lines.push_back(line);
        evtq.push_back(DerivedEvent::DisplayLines(lines));
    }
}


/// major refactor

pub fn start_from_mud_handle(evttx: Sender<Event>, from_mud: impl io::Read + Send + 'static) {
    thread::spawn(move || {
        let mut inbound = Inbound::new(from_mud, 4096);
        loop {
            match inbound.recv() {
                Err(e) => {
                    eprintln!("channel receive inbound message error {}", e);
                    return;
                }
                Ok(InboundMessage::Disconnected) => {
                    // once disconnected, stop the thread
                    break;
                }
                Ok(InboundMessage::Empty) => (),
                Ok(InboundMessage::TelnetDataToSend(bs)) => {
                    evttx.send(Event::TelnetBytesToMud(bs)).unwrap()
                }
                Ok(InboundMessage::Text(bs)) => evttx.send(Event::BytesFromMud(bs)).unwrap(),
            }
        }
    });
}

pub fn start_to_mud_handle(to_mud: impl io::Write + Send + 'static) -> Result<Sender<Vec<u8>>> {
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
    Ok(tx)
}

pub fn init_server_log(log_file: &str) -> Result<File> {
    let file = File::create(log_file)?;
    Ok(file)
}