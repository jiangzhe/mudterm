pub mod client;
pub mod server;
pub mod standalone;

use crate::codec::{AnsiBuffer, Codec, Decoder, Encoder};
use crate::conf;
use crate::error::{Error, Result};
use crate::event::{DerivedEvent, Event};
use crate::runtime::script::Script;
use crate::runtime::trigger::Trigger;
use crate::runtime::{self, Target};
use crate::signal;
use crate::style::{StyleReflector, StyledLine};
use crate::transport::{Inbound, InboundMessage, Outbound};
use crate::ui::{init_terminal, RawScreen, RawScreenCallback, RawScreenInput};
use crate::userinput;
use crossbeam_channel::{unbounded, Receiver, Sender};
use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::Write;
use std::sync::{Arc, Mutex, RwLock};
use std::{io, thread};

/// standalone app, directly connect to mud world
/// and render UI
pub struct App {
    config: conf::Config,
    // pub(crate) evtrx: Receiver<Event>,
    evttx: Sender<Event>,
    // store both tx and thread, so when exiting the application
    // we can wait for ui thread to quit the raw mode
    gcodec: Codec,
    decoder: Decoder,
    encoder: Encoder,
    reflector: StyleReflector,
    ansi_buf: AnsiBuffer,
    cmd_nr: usize,
    to_screen: Option<(Sender<RawScreenInput>, thread::JoinHandle<()>)>,
    to_mud: Option<Sender<Vec<u8>>>,
    serverlog: Option<File>,
    // script: Script,
    // triggers: Vec<Trigger>,
    // evtq: Arc<Mutex<VecDeque<DerivedEvent>>>,
}

type EventQueue = Arc<Mutex<VecDeque<DerivedEvent>>>;

impl App {
    pub fn with_config(config: conf::Config, evttx: Sender<Event>) -> Self {
        // event channel, processed by main loop
        // let (evttx, evtrx) = unbounded();
        Self {
            config,
            // evtrx,
            evttx,
            gcodec: Codec::default(),
            decoder: Decoder::default(),
            encoder: Encoder::default(),
            reflector: StyleReflector::default(),
            ansi_buf: AnsiBuffer::new(),
            cmd_nr: 0,
            // script: Script::new(),
            // triggers: Vec::new(),
            // evtq: Arc::new(Mutex::new(VecDeque::new())),
            to_screen: None,
            to_mud: None,
            serverlog: None,
        }
    }

    pub fn termconf(&self) -> conf::Term {
        self.config.term.clone()
    }

    pub fn switch_codec(&mut self, code: Codec) {
        self.gcodec = code;
        self.decoder.switch_codec(code);
        self.encoder.switch_codec(code);
    }

    // todo: refactor
    pub fn main_loop(mut self, mut evtrx: Receiver<Event>, mut script: Script, evtq: EventQueue) -> Result<()> {
        let mut serverlog = File::create(&self.config.server.log_file)?;
        let vars = Arc::new(RwLock::new(HashMap::new()));
        script
            .setup_script_functions(vars.clone(), evtq.clone())?;
        loop {
            let evt = evtrx.recv()?;
            if self.handle_event(evt, &evtq, &mut script, &mut serverlog) {
                break;
            }
            // script generated events are stored in evtq
            let events: Vec<DerivedEvent> = evtq.lock().unwrap().drain(..).collect();
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

    fn handle_event(&mut self, evt: Event, evtq: &EventQueue, script: &mut Script, logger: &mut File) -> bool {
        match evt {
            Event::Quit => return true,
            Event::NewClient(_)
            | Event::ClientAuthFail
            | Event::ClientAuthSuccess(_)
            | Event::ClientDisconnect => {
                unreachable!("standalone mode does not support certain events")
            }
            Event::Tick => {
                if let Some((ref to_screen, _)) = self.to_screen {
                    if let Err(e) = to_screen.send(RawScreenInput::Tick) {
                        eprintln!("send screen input tick error {}", e);
                    }
                }
            }
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
                    // let text = sm.orig.clone();
                    push_evtq_styled_line(&evtq, sm);
                    // invoke trigger for each line
                    // for tr in &self.triggers {
                    //     // invoke at most one matched trigger
                    //     if tr.is_match(&text) {
                    //         eprintln!("trigger matched: {}", text);
                    //         if let Err(e) = self.script.exec(&tr.model.scripts) {
                    //             eprintln!("exec script error {}", e);
                    //             // also send to event bus
                    //             self.push_evtq_styled_line(StyledLine::err(e.to_string()));
                    //         }
                    //         break;
                    //     }
                    // }
                }
            }
            Event::UserInputLine(mut cmd) => {
                // todo: might be regenerated by alias
                // todo: might be other built-in command
                if cmd.ends_with('\n') {
                    cmd.truncate(cmd.len() - 1);
                }
                // todo: add alias/script handling
                let cmds = runtime::translate_cmds(cmd, self.config.term.cmd_delimiter, &vec![]);
                for (tgt, cmd) in cmds {
                    match tgt {
                        Target::World => {
                            if self.config.term.echo_cmd && self.cmd_nr > 2 {
                                push_evtq_styled_line(&evtq, StyledLine::raw(cmd.to_owned()));
                            }
                            push_evtq_mud_string(&evtq, cmd);
                        }
                        Target::Script => {
                            if let Err(e) = script.exec(cmd) {
                                push_evtq_styled_line(&evtq, StyledLine::err(e.to_string()));
                            }
                        }
                    }
                }
            }
            Event::UserInputLines(cmds) => unimplemented!(),
            Event::UserScriptLine(s) => {
                if let Err(e) = script.exec(&s) {
                    // this action will only happen on evttx, so send it to evtq is safe
                    // let err_style = Style::default().add_modifier(Modifier::REVERSED);
                    // let err_line = StyledMessage{spans: vec![Span::styled(s.to_owned(), err_style)], orig: s, ended: true};
                    push_evtq_styled_line(&evtq, StyledLine::err(e.to_string()));
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


}

fn push_evtq_styled_line(evtq: &EventQueue, line: StyledLine) {
    let mut evtq = evtq.lock().unwrap();
    if let Some(DerivedEvent::DisplayLines(lines)) = evtq.back_mut() {
        lines.push_back(line);
        return;
    }
    let mut lines = VecDeque::new();
    lines.push_back(line);
    evtq.push_back(DerivedEvent::DisplayLines(lines));
}

fn push_evtq_mud_string(evtq: &EventQueue, cmd: String) {
    let mut evtq = evtq.lock().unwrap();
    if let Some(DerivedEvent::StringToMud(s)) = evtq.back_mut() {
        if !s.ends_with('\n') {
            s.push('\n');
        }
        s.push_str(&cmd);
        return;
    }
    evtq.push_back(DerivedEvent::StringToMud(cmd));
}

pub struct StandaloneCallback {
    evttx: Sender<Event>,
}

impl StandaloneCallback {
    pub fn new(evttx: Sender<Event>) -> Self {
        Self { evttx }
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

