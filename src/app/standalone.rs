use crate::error::Result;
use crate::event::{Event, EventHandler, NextStep, QuitHandler};
use crate::runtime::{Runtime, RuntimeOutput, RuntimeOutputHandler};
use crate::ui::line::Line;
use crate::ui::UIEvent;
use crossbeam_channel::Sender;
use std::thread;

/// standalone app, directly connect to mud world
/// and render UI
pub struct Standalone {
    uitx: Sender<UIEvent>,
    worldtx: Sender<Vec<u8>>,
}

impl Standalone {
    pub fn new(uitx: Sender<UIEvent>, worldtx: Sender<Vec<u8>>) -> Self {
        Self { uitx, worldtx }
    }
}

impl EventHandler for Standalone {
    fn on_event(&mut self, evt: Event, rt: &mut Runtime) -> Result<NextStep> {
        match evt {
            Event::Quit => return Ok(NextStep::Quit),
            // 直接发送给MUD
            Event::TelnetBytes(bs) => {
                self.worldtx.send(bs)?;
            }
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
            Event::WorldBytes(bs) => {
                rt.process_bytes_from_mud(&bs)?;
            }
            Event::WorldLines(lines) => {
                rt.process_world_lines(lines);
            }
            Event::UserOutput(output) => {
                rt.process_user_output(output);
            }
            Event::WorldDisconnected => {
                log::error!("world down or not reachable");
                // let user quit
                rt.push_line_to_ui(Line::fmt_err("与服务器断开了连接，请关闭并重新连接"));
            }
            // standalone模式不支持客户端连接，待增强
            Event::NewClient(..)
            | Event::ClientAuthFail
            | Event::ClientAuthSuccess(_)
            | Event::ClientDisconnect
            | Event::LinesFromServer(_)
            | Event::ServerDown => unreachable!("standalone mode does not support event {:?}", evt),
        }
        Ok(NextStep::Run)
    }
}

impl RuntimeOutputHandler for Standalone {
    fn on_runtime_output(&mut self, output: RuntimeOutput, rt: &mut Runtime) -> Result<NextStep> {
        match output {
            RuntimeOutput::ToServer(mut s) => {
                if !s.ends_with('\n') {
                    s.push('\n');
                }
                let bs = rt.encode(&s)?;
                self.worldtx.send(bs)?;
            }
            RuntimeOutput::ToUI(_, styled) => {
                self.uitx.send(UIEvent::Lines(styled))?;
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
