use crate::error::Result;
use crate::protocol::Packet;
use crate::signal;
use crate::style::{err_line, StyledLine};
use crate::ui::{RawScreen, RawScreenCallback, RawScreenInput};
use crate::userinput;
use crossbeam_channel::{unbounded, Sender};
use std::{io, thread};

pub struct Client {
    uitx: Sender<RawScreenInput>,
}

impl Client {
    pub fn new(uitx: Sender<RawScreenInput>) -> Self {
        Self { uitx }
    }

    pub fn start_to_server_handle(
        &mut self,
        mut to_server: impl io::Write + Send + 'static,
    ) -> Result<Sender<Packet>> {
        let uitx = self.uitx.clone();
        let (tx, rx) = unbounded::<Packet>();
        thread::spawn(move || loop {
            match rx.recv() {
                Err(e) => {
                    eprintln!("channel recv server message error {}", e);
                }
                Ok(sm) => {
                    if let Err(e) = Packet::from(sm).write_to(&mut to_server) {
                        eprintln!("failed to send packet to server {}", e);
                        uitx.send(RawScreenInput::Line(err_line(
                            "无法向服务器发送信息，请尝试关闭应用并重新连接".to_owned(),
                        )))
                        .unwrap();
                        return;
                    }
                }
            }
        });
        Ok(tx)
    }

    pub fn start_from_server_handle(&mut self, mut from_server: impl io::Read + Send + 'static) {
        let uitx = self.uitx.clone();
        thread::spawn(move || loop {
            match Packet::read_from(&mut from_server) {
                Err(e) => {
                    eprintln!("failed reading packet from server {}", e);
                    uitx.send(RawScreenInput::Line(err_line(
                        "无法从服务器接收信息，请尝试关闭应用并重新连接".to_owned(),
                    )))
                    .unwrap();
                    return;
                }
                Ok(Packet::StyledText(spans, ended)) => {
                    let mut orig = String::new();
                    for span in &spans {
                        orig.push_str(&*span.content);
                    }
                    uitx.send(RawScreenInput::Line(StyledLine { spans, orig, ended }))
                        .unwrap();
                }
                Ok(other) => eprintln!("unexpected message from server: {:?}", other),
            }
        });
    }

    pub fn start_signal_handle(&mut self) {
        let uitx = self.uitx.clone();
        thread::spawn(move || {
            if let Err(e) = signal::subscribe_signals_for_ui(uitx) {
                eprintln!("signal error {}", e);
            }
        });
    }

    pub fn start_userinput_handle(&mut self) {
        let uitx = self.uitx.clone();
        thread::spawn(move || {
            if let Err(e) = userinput::subscribe_userinput_for_ui(uitx) {
                eprintln!("userinput error {}", e);
            }
        });
    }
}

pub struct ClientCallback {
    srvtx: Sender<Packet>,
}

impl ClientCallback {
    pub fn new(srvtx: Sender<Packet>) -> Self {
        Self { srvtx }
    }
}

impl RawScreenCallback for ClientCallback {
    fn on_cmd(&mut self, _term: &mut RawScreen, cmd: String) {
        if let Err(e) = self.srvtx.send(Packet::Text(cmd)) {
            eprintln!("channel send server message error {}", e);
        }
    }

    fn on_script(&mut self, term: &mut RawScreen, script: String) {
        self.on_cmd(term, script);
    }

    fn on_quit(&mut self, _term: &mut RawScreen) {}
}
