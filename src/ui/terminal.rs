use crate::error::Result;
use crate::ui::buffer::BufferVec;
use crate::ui::buffer::{Buffer, Cell};
use crate::ui::layout::Rect;
use crate::ui::widget::Widget;
use std::io::Write;
use std::io::{self, Stdout};
use termion::input::MouseTerminal;
use termion::raw::{IntoRawMode, RawTerminal};
use termion::screen::AlternateScreen;
use termion::terminal_size;

/// wrapped termion's alternate screen with mouse support
pub struct Terminal {
    out: AlternateScreen<MouseTerminal<RawTerminal<Stdout>>>,
    curr_buf: BufferVec,
    prev_buf: BufferVec,
    size: (u16, u16),
}

impl Terminal {
    pub fn init() -> Result<Self> {
        let out = io::stdout().into_raw_mode()?;
        let out = MouseTerminal::from(out);
        let out = AlternateScreen::from(out);
        let (width, height) = terminal_size()?;
        let rect = Rect {
            x: 1,
            y: 1,
            width,
            height,
        };
        Ok(Self {
            out,
            curr_buf: BufferVec::empty(rect),
            prev_buf: BufferVec::empty(rect),
            size: (width, height),
        })
    }

    pub fn size(&self) -> (u16, u16) {
        self.size
    }

    pub fn render_widget<W: Widget>(
        &mut self,
        widget: &mut W,
        area: Rect,
        cjk: bool,
    ) -> Result<()> {
        let curr_buffer = self.curr_buf_mut();
        let mut subset = curr_buffer.subset(area)?;
        widget.refresh_buffer(&mut subset, cjk)?;
        Ok(())
    }

    fn curr_buf_mut(&mut self) -> &mut BufferVec {
        &mut self.curr_buf
    }

    pub fn flush(&mut self, area: Rect) -> Result<()> {
        let prev_buf = &self.prev_buf.subset(area)?;
        let curr_buf = &self.curr_buf.subset(area)?;
        let updates = prev_buf.diff(curr_buf);
        // eprintln!("updates={:#?}", updates);
        draw_updates(&mut self.out, updates)?;
        self.out.flush()?;
        std::mem::swap(&mut self.prev_buf, &mut self.curr_buf);
        self.curr_buf.reset();
        Ok(())
    }

    pub fn set_cursor(&mut self, x: u16, y: u16) -> Result<()> {
        write!(self.out, "{}", termion::cursor::Goto(x, y))?;
        self.out.flush()?;
        Ok(())
    }
}

fn draw_updates<W: Write>(out: &mut W, updates: Vec<(u16, u16, &Cell)>) -> Result<()> {
    let mut next_x: u16 = 0;
    let mut next_y: u16 = 0;
    for (x, y, cell) in updates {
        if x != next_x || y != next_y {
            write!(out, "{}", termion::cursor::Goto(x, y))?;
        }
        write!(out, "{}", cell.symbol.ch)?;
        next_x = x;
        next_y = y + cell.symbol.width as u16;
    }
    Ok(())
}

impl Write for Terminal {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.out.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.out.flush()
    }
}
