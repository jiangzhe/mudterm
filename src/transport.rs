use crate::error::Result;
use libtelnet_rs::events::{TelnetEvents, TelnetIAC, TelnetNegotiation, TelnetSubnegotiation};
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
        let telnet = Parser::with_capacity(4096);
        Self {
            reader,
            recv_buf: vec![0u8; buf_size],
            parser: telnet,
            buf: VecDeque::new(),
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
        // let mut text = vec![];
        for event in events {
            match event {
                TelnetEvents::IAC(TelnetIAC { command }) => {
                    log::trace!("TelnetIAC[command={}]", command);
                }
                TelnetEvents::Negotiation(TelnetNegotiation { command, option }) => {
                    log::trace!("TelnetNegotiation[command={}, option={}]", command, option);
                }
                TelnetEvents::DataReceive(bs) => {
                    log::trace!("TelnetDataReceive[len={}]", bs.len());
                    self.buf.push_back(TelnetEvent::Text(bs));
                }
                TelnetEvents::DataSend(bs) => {
                    log::trace!("TelnetDataSend[len={}]", bs.len());
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
            // encoder: Encoder::default(),
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
