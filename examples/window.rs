use mudterm::error::{Error, Result};
use mudterm::ui::terminal::Terminal;
use mudterm::ui::ansi::SpanStream;
use mudterm::ui::line::{RawLine, RawLines};
use std::io::{Write, Read, stdin};
use termion::input::{TermRead, MouseTerminal};
use termion::event::{Event, Key, MouseEvent, MouseButton};
use std::collections::VecDeque;
use gag::Redirect;
use std::fs::File;

fn main() -> Result<()> {
    let debuglog = File::create("window_debug.log")?;
    let _stderr_redirect = Redirect::stderr(debuglog)
        .map_err(|e| Error::RuntimeError(format!("Redirect stderr error {}", e)))?;
    
    let stdin = stdin();
    let mut terminal = Terminal::init()?;
    let mut buf = String::new();
    let mut lines = RawLines::unbounded();
    let mut spans = SpanStream::new();
    spans.fill(read_file()?);
    while let Some(span) = spans.next_span() {
        eprintln!("span={:?}", span);
        let s = if span.ended() {
            let mut s = span.content().to_owned();
            s.push_str("\r\n");
            s
        } else {
            span.content().to_owned()
        };
        lines.push_line(RawLine::owned(s));
    }

    for key in stdin.keys() {
        match key? {
            Key::Ctrl('q') => break,
                Key::Char('\n') => {
                    let mut s = std::mem::replace(&mut buf, String::new());
                    s.push_str("\r\n");
                    // write!(terminal, "\r\n")?;
                    let line = RawLine::owned(s);
                    lines.push_line(line);
                }
                Key::Char(c) => {
                    buf.push(c);
                }
                Key::Up => {
                    write!(terminal, "{}", termion::scroll::Up(1))?;
                    terminal.flush()?;
                }
                Key::Down => {
                    write!(terminal, "{}", termion::scroll::Down(1))?;
                    terminal.flush()?;
                }
                _ => (),
        }
        render(&mut terminal, lines.to_vec(), &buf)?;
    }
    Ok(())
}

fn render<W: Write>(writer: &mut W, lines: Vec<RawLine>, buf: &str) -> Result<()> {
    for line in lines {
        write!(writer, "{}", line.as_ref())?;
    }
    // 2 empty lines
    write!(writer, "\r\n{}", buf)?;
    writer.flush()?;
    Ok(())
}

fn read_file() -> Result<String> {
    let mut s = String::new();
    let mut f = File::open("server.log")?;
    f.read_to_string(&mut s)?;
    Ok(s)
}