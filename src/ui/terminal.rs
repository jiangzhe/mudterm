use std::io::{self, Stdout};
use termion::input::MouseTerminal;
use termion::raw::{IntoRawMode, RawTerminal};
use termion::screen::AlternateScreen;
use std::io::Write;
use crate::error::Result;

/// wrapped termion's alternate screen with mouse support
pub struct Terminal(AlternateScreen<MouseTerminal<RawTerminal<Stdout>>>);

impl Terminal {
    pub fn init() -> Result<Self> {
        let stdout = io::stdout().into_raw_mode()?;
        let stdout = MouseTerminal::from(stdout);
        let stdout = AlternateScreen::from(stdout);
        Ok(Self(stdout))
    }
}

impl Write for Terminal {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

