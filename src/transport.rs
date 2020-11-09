use crate::error::Result;
use libtelnet_rs::events::{TelnetEvents, TelnetIAC, TelnetNegotiation, TelnetSubnegotiation};
use libtelnet_rs::Parser;
use std::collections::VecDeque;
use std::io::{Read, Write};

#[derive(Debug, Clone)]
pub enum InboundMessage {
    Text(Vec<u8>),
    TelnetDataToSend(Vec<u8>),
    Empty,
    Disconnected,
}

pub struct Inbound<R> {
    reader: R,
    recv_buf: Vec<u8>,
    telnet: Parser,
    buf: VecDeque<InboundMessage>,
}

impl<R> Inbound<R>
where
    R: Read,
{
    pub fn new(reader: R, buf_size: usize) -> Self {
        let telnet = Parser::with_capacity(4096);
        Self {
            reader,
            recv_buf: vec![0u8; buf_size],
            telnet,
            buf: VecDeque::new(),
        }
    }

    pub fn recv(&mut self) -> Result<InboundMessage> {
        // pop from buffer first
        if let Some(msg) = self.buf.pop_front() {
            return Ok(msg);
        }
        // then try receive from server
        let n = self.reader.read(&mut self.recv_buf[..])?;
        if n == 0 {
            return Ok(InboundMessage::Disconnected);
        }
        let events = self.telnet.receive(&self.recv_buf[..n]);
        // let mut text = vec![];
        for event in events {
            match event {
                TelnetEvents::IAC(TelnetIAC { command }) => {
                    eprintln!("TelnetIAC[command={}]", command);
                }
                TelnetEvents::Negotiation(TelnetNegotiation { command, option }) => {
                    eprintln!("TelnetNegotiation[command={}, option={}]", command, option);
                }
                TelnetEvents::DataReceive(bs) => {
                    // text.extend(bs);
                    self.buf.push_back(InboundMessage::Text(bs));
                }
                TelnetEvents::DataSend(bs) => {
                    // eprintln!("TelnetDataSend={:?}", bs);
                    self.buf.push_back(InboundMessage::TelnetDataToSend(bs));
                }
                TelnetEvents::Subnegotiation(TelnetSubnegotiation { option, buffer }) => {
                    eprintln!(
                        "TelnetSubnegotiation[option={}, buffer={:?}]",
                        option, buffer
                    );
                }
                _ => (),
            }
        }
        if let Some(msg) = self.buf.pop_front() {
            return Ok(msg);
        }
        Ok(InboundMessage::Empty)
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
