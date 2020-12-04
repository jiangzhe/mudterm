use crate::error::Result;
use libtelnet_rs::events::{TelnetEvents, TelnetIAC, TelnetNegotiation, TelnetSubnegotiation};
use libtelnet_rs::compatibility::CompatibilityTable;
use libtelnet_rs::telnet::op_command as Op;
use libtelnet_rs::Parser;
use std::collections::VecDeque;
use std::io::{Read, Write};

#[derive(Debug, Clone)]
pub enum TelnetEvent {
    Text(Vec<u8>),
    DataToSend(Vec<u8>),
    Empty,
    Disconnected,
}

pub struct Telnet<R> {
    reader: R,
    recv_buf: Vec<u8>,
    parser: Parser,
    buf: VecDeque<TelnetEvent>,
}

impl<R> Telnet<R>
where
    R: Read,
{
    pub fn new(reader: R, buf_size: usize) -> Self {
        let compat_table = CompatibilityTable::new();
        // compat_table.support_local(86);
        // compat_table.support_remote(86);
        // compat_table.support_local(91);
        // compat_table.support_remote(91);
        let telnet = Parser::with_support_and_capacity(4096, compat_table);
        let buf = VecDeque::new();
        Self {
            reader,
            recv_buf: vec![0u8; buf_size],
            parser: telnet,
            buf,
        }
    }

    pub fn recv(&mut self) -> Result<TelnetEvent> {
        // pop from buffer first
        if let Some(msg) = self.buf.pop_front() {
            return Ok(msg);
        }
        // then try receive from server
        let n = self.reader.read(&mut self.recv_buf[..])?;
        if n == 0 {
            return Ok(TelnetEvent::Disconnected);
        }
        let events = self.parser.receive(&self.recv_buf[..n]);
        for event in events {
            match event {
                TelnetEvents::IAC(TelnetIAC { command }) => {
                    log::trace!("TelnetIAC[command={}]", command);
                }
                TelnetEvents::Negotiation(TelnetNegotiation { command, option }) => {
                    log::trace!("TelnetNegotiation[command={}, option={}]", command, option);
                }
                TelnetEvents::DataReceive(bs) => {
                    self.buf.push_back(TelnetEvent::Text(bs));
                }
                TelnetEvents::DataSend(bs) => {
                    self.buf.push_back(TelnetEvent::DataToSend(bs));
                }
                TelnetEvents::Subnegotiation(TelnetSubnegotiation { option, buffer }) => {
                    log::trace!(
                        "TelnetSubnegotiation[option={}, buffer={:?}]",
                        option,
                        buffer
                    );
                }
                _ => (),
            }
        }
        if let Some(msg) = self.buf.pop_front() {
            return Ok(msg);
        }
        Ok(TelnetEvent::Empty)
    }
}

pub struct Outbound<W> {
    writer: W,
}

impl<W> Outbound<W>
where
    W: Write,
{
    pub fn new(writer: W) -> Self {
        Self {
            writer,
        }
    }

    pub fn send(&mut self, bs: Vec<u8>) -> Result<()> {
        self.writer.write_all(&bs)?;
        self.writer.flush()?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum WorldInput {
    Bytes(Vec<u8>),
}
