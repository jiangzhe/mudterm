use crate::codec::Codec;
use crate::style::StyledLine;
use std::collections::VecDeque;
use std::net::TcpStream;
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
    /// stacked user input lines
    UserInputLines(Vec<String>),
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
    NewClient(TcpStream),
    // client authentication fail
    ClientAuthFail,
    // client authentication success
    ClientAuthSuccess(TcpStream),
    // client disconnect
    ClientDisconnect,
    // terminal key event
    TerminalKey(Key),
    // terminal mouse event
    TerminalMouse(MouseEvent),
}

#[derive(Debug, Clone)]
pub enum DerivedEvent {
    /// switch codec for both encoding and decoding
    SwitchCodec(Codec),
    /// string which is to be sent to server
    StringToMud(String),
    /// lines from server or script to display
    DisplayLines(VecDeque<StyledLine>),
}
