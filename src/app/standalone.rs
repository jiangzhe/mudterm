use crate::transport::{Inbound, Outbound, InboundMessage};
use crate::signal;
use crate::userinput;
use crate::ui::{RawScreenInput, RawScreen, render_ui, RawScreenCallback};
use crate::error::{Result, Error};
use crate::style::{StyledLine, StyleReflector, err_line};
use crate::codec::{Codec, Decoder, Encoder, AnsiBuffer};
use crate::script::Script;
use crate::event::{Event, DerivedEvent};
use crate::conf;
use crate::trigger::{CompiledTrigger};
use crossbeam_channel::{Receiver, Sender, unbounded};
use std::{io, thread};
use std::io::Write;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::fs::File;

/// standalone app, directly connect to mud world
/// and render UI
pub struct Standalone {
    config: conf::Config,
    evtrx: Receiver<Event>,
    evttx: Sender<Event>,
    // store both tx and thread, so when exiting the application
    // we can wait for ui thread to quit the raw mode
    to_screen: Option<(Sender<RawScreenInput>, thread::JoinHandle<()>)>,
    to_mud: Option<Sender<Vec<u8>>>,
    gcodec: Codec,
    decoder: Decoder,
    encoder: Encoder,
    reflector: StyleReflector,
    script: Script,
    triggers: Vec<CompiledTrigger>,
    evtq: Arc<Mutex<VecDeque<DerivedEvent>>>,
    cmd_nr: usize,
    ansi_buf: AnsiBuffer,
}

impl Standalone {

    pub fn with_config(config: conf::Config) -> Self {
        // event channel, processed by main loop
        let (evttx, evtrx) = unbounded();
        Self {
            config,
            evtrx,
            evttx,
            to_screen: None,
            to_mud: None,
            gcodec: Codec::default(),
            decoder: Decoder::default(),
            encoder: Encoder::default(),
            reflector: StyleReflector::default(),
            script: Script::new(),
            triggers: Vec::new(),
            evtq: Arc::new(Mutex::new(VecDeque::new())),
            cmd_nr: 0,
            ansi_buf: AnsiBuffer::new(),
        }
    }

    pub fn switch_codec(&mut self, code: Codec) {
        self.gcodec = code;
        self.decoder.switch_codec(code);
        self.encoder.switch_codec(code);
    }

    pub fn load_triggers(&mut self) -> Result<()> {
        Ok(())
    }

    pub fn start_userinput_handle(&mut self) {
        let evttx = self.evttx.clone();
        thread::spawn(move || {
            if let Err(e) = userinput::subscribe_userinput(evttx) {
                eprintln!("userinput error {}", e);
            }
        });
    }

    pub fn start_from_mud_handle(&mut self, from_mud: std::net::TcpStream) {
        let evttx = self.evttx.clone();
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
                    },
                    Ok(InboundMessage::Empty) => (),
                    Ok(InboundMessage::TelnetDataToSend(bs)) => evttx.send(Event::TelnetBytesToMud(bs)).unwrap(),
                    Ok(InboundMessage::Text(bs)) => evttx.send(Event::BytesFromMud(bs)).unwrap(),
                }
            }
        });
    }

    pub fn start_to_mud_handle(&mut self, to_mud: impl io::Write + Send + 'static) -> Result<()> {
        if self.to_mud.is_some() {
            return Err(Error::RuntimeError("already have connection to mud".to_owned()));
        }
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
        self.to_mud = Some(tx);
        Ok(())
    }

    pub fn start_signal_handle(&mut self) {
        let evttx = self.evttx.clone();
        thread::spawn(move || {
            if let Err(e) = signal::subscribe_signals(evttx) {
                eprintln!("signal error {}", e);
            }
        });
    }

    pub fn start_ui_handle(&mut self) -> Result<()> {
        if self.to_screen.is_some() {
            return Err(Error::RuntimeError("already have screen handling thread".to_owned()));
        }
        let (uitx, uirx) = unbounded::<RawScreenInput>();
        let evttx = self.evttx.clone();
        let termconf = self.config.term.clone();
        let handle = thread::spawn(move || {
            let term = RawScreen::new(termconf);
            let cb = StandaloneCallback::new(evttx);
            if let Err(e) = render_ui(term, uirx, cb) {
                eprintln!("screen error {}", e);
            }
        });
        self.to_screen = Some((uitx, handle));
        Ok(())
    }

    pub fn main_loop(mut self) -> Result<()> {
        let mut serverlog = File::create(&self.config.server.log_file)?;
        let vars = Arc::new(RwLock::new(HashMap::new()));
        self.script.setup_script_functions(vars.clone(), self.evtq.clone())?;
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
        if let Some((_, ui_handle)) = self.to_screen.take() {
            if let Err(e) = ui_handle.join() {
                eprintln!("UI thread exits with error {:?}", e);
            }
        }
        Ok(())
    }

    fn handle_event(&mut self, evt: Event, logger: &mut File) -> bool {
        match evt {
            Event::Quit => return true,
            Event::NewClient(_) | Event::ClientAuthFail | Event::ClientAuthSuccess(_) | Event::ClientDisconnect => unreachable!("standalone mode does not support certain events"),
            Event::Tick => {
                if let Some((ref to_screen, _)) = self.to_screen {
                    if let Err(e) = to_screen.send(RawScreenInput::Tick) {
                        eprintln!("send screen input tick error {}", e);
                    }
                }
            },
            Event::TerminalKey(k) => {
                if let Some((ref to_screen, _)) = self.to_screen {
                    if let Err(e) = to_screen.send(RawScreenInput::Key(k)) {
                        eprintln!("send screen input key error {}", e);
                    }
                }
            }
            Event::TerminalMouse(m) => {
                if let Some((ref to_screen, _)) = self.to_screen {
                    if let Err(e) = to_screen.send(RawScreenInput::Mouse(m)) {
                        eprintln!("send screen input mouse error {}", e);
                    }
                }
            }
            Event::WindowResize => {
                if let Some((ref to_screen, _)) = self.to_screen {
                    if let Err(e) = to_screen.send(RawScreenInput::WindowResize) {
                        eprintln!("send screen input windowresize error {}", e);
                    }
                }
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
                if let Err(e) = self.evttx.send(Event::StyledLinesFromMud(sms)) {
                    eprintln!("send server styled lines error {}", e);
                }
            }
            Event::StyledLinesFromMud(sms) => {
                for sm in sms {
                    let text = sm.orig.clone();
                    self.push_evtq_styled_line(sm);
                    // invoke trigger for each line
                    for ctr in &self.triggers {
                        // invoke at most one matched trigger
                        if ctr.is_match(&text) {
                            eprintln!("trigger matched: {}", text);
                            if let Err(e) = self.script.exec(&ctr.content) {
                                eprintln!("exec script error {}", e);
                                // also send to event bus
                                self.push_evtq_styled_line(err_line(e.to_string()));
                            }
                            break;
                        }
                    }
                }
            }
            Event::UserInputLine(mut cmd) => {
                // todo: might be regenerated by alias
                // todo: might be other built-in command
                if cmd.ends_with('\n') {
                    cmd.truncate(cmd.len()-1);
                }
                let sep = self.config.term.cmd_delimiter;
                let cmds: Vec<String> = cmd.split(|c| c == '\n' || c == sep).filter(|s| !s.is_empty()).map(|s| s.to_owned()).collect();
                for mut cmd in cmds {
                    if cmd.is_empty() && self.config.term.ignore_empty_cmd {
                        continue;
                    }
                    self.cmd_nr += 1;
                    if self.config.term.echo_cmd && self.cmd_nr > 2 {
                        if let Some((ref to_screen, _)) = self.to_screen {
                            if let Err(e) = to_screen.send(RawScreenInput::Line(StyledLine::raw_line(cmd.to_owned()))) {
                                eprintln!("send echo command error {}", e);
                            }
                        }
                    }
                    // add newline at end and send to world
                    cmd.push('\n');
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
            }
            Event::UserInputLines(cmds) => {
                unimplemented!()
            }
            Event::UserScriptLine(s) => {
                if let Err(e) = self.script.exec(&s) {
                    // this action will only happen on evttx, so send it to evtq is safe
                    // let err_style = Style::default().add_modifier(Modifier::REVERSED);
                    // let err_line = StyledMessage{spans: vec![Span::styled(s.to_owned(), err_style)], orig: s, ended: true};
                    self.push_evtq_styled_line(err_line(e.to_string()));
                }
            }
            Event::TelnetBytesToMud(bs) => {
                if let Some(ref to_mud) = self.to_mud {
                    if let Err(e) = to_mud.send(bs) {
                        eprintln!("send telnet bytes error {}", e);
                    }
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
                if let Some((ref to_screen, _)) = self.to_screen {
                    if let Err(e) = to_screen.send(RawScreenInput::Lines(sms)) {
                        eprintln!("send screen input lines(script) error {}", e);
                    }
                }
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

pub struct StandaloneCallback {
    evttx: Sender<Event>,
}

impl StandaloneCallback {
    pub fn new(evttx: Sender<Event>) -> Self {
        Self{evttx}
    }
}

impl RawScreenCallback for StandaloneCallback {

    fn on_cmd(&mut self, _term: &mut RawScreen, cmd: String) {
        self.evttx.send(Event::UserInputLine(cmd)).unwrap();
    }

    fn on_script(&mut self, _term: &mut RawScreen, script: String) {
        self.evttx.send(Event::UserScriptLine(script)).unwrap();
    }

    fn on_quit(&mut self, _term: &mut RawScreen) {
        self.evttx.send(Event::Quit).unwrap();
    }
}