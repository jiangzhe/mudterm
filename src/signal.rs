use crossbeam_channel::Sender;
use crate::error::Result;
use signal_hook::iterator::Signals;
use crate::event::Event;
use crate::ui::RawScreenInput;

pub fn subscribe_signals(tx: Sender<Event>) -> Result<()> {
    let sigs = Signals::new(&[
        signal_hook::SIGWINCH,
    ])?;
    loop {
        for sig in sigs.pending() {
            match sig as libc::c_int {
                signal_hook::SIGWINCH => {
                    tx.send(Event::WindowResize)?;
                }
                _ => (),
            }
        }
    }
}

pub fn subscribe_signals_for_ui(tx: Sender<RawScreenInput>) -> Result<()> {
    let sigs = Signals::new(&[
        signal_hook::SIGWINCH,
    ])?;
    loop {
        for sig in sigs.pending() {
            match sig as libc::c_int {
                signal_hook::SIGWINCH => {
                    tx.send(RawScreenInput::WindowResize)?;
                }
                _ => (),
            }
        }
    }
}