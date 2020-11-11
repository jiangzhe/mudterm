use crate::error::Result;
use crate::event::{Event, EventHandler, NextStep, QuitHandler, RuntimeEvent, RuntimeEventHandler};
use crate::runtime::Runtime;
use crate::ui::RawScreenInput;
use crossbeam_channel::Sender;
use std::thread;

/// standalone app, directly connect to mud world
/// and render UI
pub struct Standalone {
    uitx: Sender<RawScreenInput>,
    worldtx: Sender<Vec<u8>>,
}

impl Standalone {
    pub fn new(uitx: Sender<RawScreenInput>, worldtx: Sender<Vec<u8>>) -> Self {
        Self { uitx, worldtx }
    }
}

impl EventHandler for Standalone {
    fn on_event(&mut self, evt: Event, rt: &mut Runtime) -> Result<NextStep> {
        match evt {
            Event::Quit => return Ok(NextStep::Quit),
            // 直接发送给MUD
            Event::TelnetBytesToMud(bs) => {
                self.worldtx.send(bs)?;
            }
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
            Event::BytesFromMud(bs) => {
                rt.process_bytes_from_mud(&bs)?;
            }
            Event::StyledLinesFromMud(lines) => {
                rt.process_mud_lines(lines);
            }
            Event::UserInputLine(cmd) => {
                rt.preprocess_user_cmd(cmd);
            }
            Event::UserScriptLine(s) => {
                rt.process_user_scripts(s);
            }
            // standalone模式不支持客户端连接，待增强
            Event::NewClient(..)
            | Event::ClientAuthFail
            | Event::ClientAuthSuccess(_)
            | Event::ClientDisconnect
            | Event::StyledLineFromServer(_)
            | Event::ServerDown => unreachable!("standalone mode does not support event {:?}", evt),
        }
        Ok(NextStep::Run)
    }
}

impl RuntimeEventHandler for Standalone {
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
                self.uitx.send(RawScreenInput::Lines(lines))?;
            }
        }
        Ok(NextStep::Run)
    }
}

pub struct QuitStandalone(Option<thread::JoinHandle<()>>);

impl QuitStandalone {
    pub fn new(handle: thread::JoinHandle<()>) -> Self {
        Self(Some(handle))
    }
}

impl QuitHandler for QuitStandalone {
    fn on_quit(&mut self) {
        if let Some(handle) = self.0.take() {
            handle.join().unwrap();
        }
    }
}
