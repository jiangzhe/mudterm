use crate::event::Event;
use crate::error::Result;
use crate::ui::RawScreenInput;
use termion::event::Event as TEvent;
use termion::input::TermRead;
use std::io;
use crossbeam_channel::Sender;

pub fn subscribe_userinput(tx: Sender<Event>) -> Result<()> {
    let stdin = io::stdin();
    for evt in stdin.events() {
        match evt? {
            TEvent::Key(key) => {
                tx.send(Event::TerminalKey(key)).unwrap();
            }
            TEvent::Mouse(mouse) => {
                tx.send(Event::TerminalMouse(mouse)).unwrap();
            }
            _ => (),
        }
    }
    Ok(())
}

pub fn subscribe_userinput_for_ui(tx: Sender<RawScreenInput>) -> Result<()> {
    let stdin = io::stdin();
    for evt in stdin.events() {
        match evt? {
            TEvent::Key(key) => {
                tx.send(RawScreenInput::Key(key)).unwrap();
            }
            TEvent::Mouse(mouse) => {
                tx.send(RawScreenInput::Mouse(mouse)).unwrap();
            }
            _ => (),
        }
    }
    Ok(())
}