use crate::error::Result;
use crate::runtime::{Engine, RuntimeOutputHandler};
use crate::runtime::timer::Timer;
use crate::runtime::delay_queue::Delay;
use crate::ui::line::RawLine;
use crate::ui::UserOutput;
use crossbeam_channel::Receiver;
use std::net::{SocketAddr, TcpStream};
use termion::event::{Key, MouseEvent};

#[derive(Debug)]
pub enum Event {
    /// raw bytes received from server
    /// decode it in main loop so that we can
    /// handle codec switching peacefully
    WorldBytes(Vec<u8>),
    /// lines from server with tui style
    // StyledLinesFromMud(VecDeque<StyledLine>),
    // WorldLines(Vec<RawLine>),
    // world disconnected, e.g idle for a lone time
    WorldDisconnected,
    /// user input line
    UserOutput(UserOutput),
    /// user script line will be sent to script
    // UserScriptLine(String),
    /// window resize event
    WindowResize,
    /// timer event
    Timer(Delay<Timer>),
    /// Quit
    Quit,
    /// raw bytes following telnet protocol, should
    /// be sent to server directly
    TelnetBytes(Vec<u8>),
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
    LinesFromServer(Vec<RawLine>),
    // terminal key event
    TerminalKey(Key),
    // terminal mouse event
    TerminalMouse(MouseEvent),
}

/// 事件回调
pub trait EventHandler {
    fn on_event(&mut self, evt: Event, engine: &mut Engine) -> Result<NextStep>;
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
    engine: Engine,
    evtrx: Receiver<Event>,
    evt_hdl: EH,
    qt_hdl: QH,
}

impl<EH, QH> EventLoop<EH, QH>
where
    EH: EventHandler + RuntimeOutputHandler,
    QH: QuitHandler,
{
    pub fn new(engine: Engine, evtrx: Receiver<Event>, evt_hdl: EH, qt_hdl: QH) -> Self {
        Self {
            engine,
            evtrx,
            evt_hdl,
            qt_hdl,
        }
    }

    pub fn run(mut self) -> Result<()> {
        'outer: loop {
            let evt = self.evtrx.recv()?;
            // 处理总线上的事件
            match self.evt_hdl.on_event(evt, &mut self.engine)? {
                NextStep::Quit => break,
                NextStep::Skip => continue,
                NextStep::Run => (),
            }
            let outputs = self.engine.apply();
            if outputs.is_empty() {
                continue;
            }
            // 处理运行时（衍生）事件
            for output in outputs {
                match self.evt_hdl.on_runtime_output(output)? {
                    NextStep::Quit => break 'outer,
                    _ => (),
                }
            }
        }
        self.qt_hdl.on_quit();
        Ok(())
    }
}
