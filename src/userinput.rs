use crate::error::Result;
use crate::event::Event;
use crate::ui::window::WindowEvent;
use crossbeam_channel::Sender;
use std::io;
use termion::event::Event as TEvent;
use termion::input::TermRead;

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

pub fn subscribe_userinput_for_ui(tx: Sender<WindowEvent>) -> Result<()> {
    let stdin = io::stdin();
    for evt in stdin.events() {
        match evt? {
            TEvent::Key(key) => {
                tx.send(WindowEvent::Key(key)).unwrap();
            }
            TEvent::Mouse(mouse) => {
                tx.send(WindowEvent::Mouse(mouse)).unwrap();
            }
            _ => (),
        }
    }
    Ok(())
}
