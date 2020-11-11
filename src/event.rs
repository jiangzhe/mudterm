use crate::codec::Codec;
use crate::error::Result;
use crate::runtime::Runtime;
use crate::style::StyledLine;
use crossbeam_channel::{Receiver, Sender};
use std::collections::VecDeque;
use std::net::{SocketAddr, TcpStream};
use std::sync::{Arc, Mutex};
use termion::event::{Key, MouseEvent};

#[derive(Debug)]
pub enum Event {
    /// raw bytes received from server
    /// decode it in main loop so that we can
    /// handle codec switching peacefully
    BytesFromMud(Vec<u8>),
    /// lines from server with tui style
    StyledLinesFromMud(VecDeque<StyledLine>),
    /// user input line
    UserInputLine(String),
    /// user script line will be sent to script
    UserScriptLine(String),
    /// window resize event
    WindowResize,
    /// tick event
    Tick,
    /// Quit
    Quit,
    /// raw bytes following telnet protocol, should
    /// be sent to server directly
    TelnetBytesToMud(Vec<u8>),
    // new client connected
    NewClient(TcpStream, SocketAddr),
    // client authentication fail
    ClientAuthFail,
    // client authentication success
    ClientAuthSuccess(TcpStream),
    // client disconnect
    ClientDisconnect,
    // server down
    ServerDown,
    // lines from server with tui style
    StyledLineFromServer(StyledLine),
    // terminal key event
    TerminalKey(Key),
    // terminal mouse event
    TerminalMouse(MouseEvent),
}

/// 运行时事件，进由运行时进行处理
///
/// 这些事件是由外部事件派生或由脚本运行生成出来
#[derive(Debug, Clone)]
pub enum RuntimeEvent {
    /// switch codec for both encoding and decoding
    SwitchCodec(Codec),
    /// string which is to be sent to server
    StringToMud(String),
    /// lines from server or script to display
    DisplayLines(VecDeque<StyledLine>),
}

/// 事件总线
pub type EventBus = Sender<Event>;

/// 运行时时间队列
#[derive(Debug, Clone)]
pub struct EventQueue(Arc<Mutex<VecDeque<RuntimeEvent>>>);

impl EventQueue {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(VecDeque::new())))
    }

    pub fn push_styled_line(&self, line: StyledLine) {
        let mut evtq = self.0.lock().unwrap();
        if let Some(RuntimeEvent::DisplayLines(lines)) = evtq.back_mut() {
            lines.push_back(line);
            return;
        }
        let mut lines = VecDeque::new();
        lines.push_back(line);
        evtq.push_back(RuntimeEvent::DisplayLines(lines));
    }

    pub fn push_mud_string(&self, cmd: String) {
        let mut evtq = self.0.lock().unwrap();
        if let Some(RuntimeEvent::StringToMud(s)) = evtq.back_mut() {
            if !s.ends_with('\n') {
                s.push('\n');
            }
            s.push_str(&cmd);
            return;
        }
        evtq.push_back(RuntimeEvent::StringToMud(cmd));
    }

    pub fn drain_all(&self) -> Vec<RuntimeEvent> {
        self.0.lock().unwrap().drain(..).collect()
    }

    pub fn push_back(&self, re: RuntimeEvent) {
        self.0.lock().unwrap().push_back(re);
    }
}

/// 事件回调
pub trait EventHandler {
    fn on_event(&mut self, evt: Event, rt: &mut Runtime) -> Result<NextStep>;
}

/// 运行时事件回调
pub trait RuntimeEventHandler {
    fn on_runtime_event(&mut self, evt: RuntimeEvent, rt: &mut Runtime) -> Result<NextStep>;
}

/// 退出回调
pub trait QuitHandler {
    fn on_quit(&mut self);
}

/// 事件处理返回的结果，指示下一步如何行动
pub enum NextStep {
    Run,
    Skip,
    Quit,
}

/// 事件循环
pub struct EventLoop<EH, QH> {
    rt: Runtime,
    evtrx: Receiver<Event>,
    evt_hdl: EH,
    qt_hdl: QH,
}

impl<EH, QH> EventLoop<EH, QH>
where
    EH: EventHandler + RuntimeEventHandler,
    QH: QuitHandler,
{
    pub fn new(rt: Runtime, evtrx: Receiver<Event>, evt_hdl: EH, qt_hdl: QH) -> Self {
        Self {
            rt,
            evtrx,
            evt_hdl,
            qt_hdl,
        }
    }

    pub fn run(mut self) -> Result<()> {
        loop {
            let evt = self.evtrx.recv()?;
            // 处理总线上的事件
            match self.evt_hdl.on_event(evt, &mut self.rt)? {
                NextStep::Quit => break,
                NextStep::Skip => continue,
                NextStep::Run => (),
            }
            // 处理运行时（衍生）事件
            match self.rt.process_runtime_events(&mut self.evt_hdl)? {
                NextStep::Quit => break,
                _ => (),
            }
        }
        self.qt_hdl.on_quit();
        Ok(())
    }
}
