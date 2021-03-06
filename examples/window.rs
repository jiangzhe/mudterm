use gag::Redirect;
use mudterm::error::{Error, Result};
use mudterm::proto::{Parser, Element};
use mudterm::ui::buffer::{Buffer, BufferVec};
use mudterm::ui::layout::Rect;
use mudterm::ui::line::{Line, RawLine, RawLines};
use mudterm::ui::style::{Color, Style};
use mudterm::ui::symbol::HORIZONTAL;
use mudterm::ui::terminal::Terminal;
use mudterm::ui::widget::cmdbar::CmdBar;
use mudterm::ui::widget::{Block, Flow, Widget};
use std::collections::VecDeque;
use std::fs::File;
use std::io::{stdin, Read, Write};
use termion::cursor::DetectCursorPos;
use termion::event::{Event, Key, MouseButton, MouseEvent};
use termion::input::{MouseTerminal, TermRead};
use termion::raw::IntoRawMode;
use termion::screen::AlternateScreen;
use termion::terminal_size;

fn main() -> Result<()> {
    // run1()
    // run2()
    // run3()
    // run4()
    // run5()
    run6()
    // run7()
}

#[allow(dead_code)]
fn write_style<W: Write>(writer: &mut W, style: Style) -> Result<()> {
    write!(writer, "{}", style)?;
    Ok(())
}

#[allow(dead_code)]
fn run7() -> Result<()> {
    let debuglog = File::create("window_debug.log")?;
    let _stderr_redirect = Redirect::stderr(debuglog)
        .map_err(|e| Error::RuntimeError(format!("Redirect stderr error {}", e)))?;
    let stdin = stdin();
    let out = std::io::stdout().into_raw_mode()?;
    let out = MouseTerminal::from(out);
    let mut out = AlternateScreen::from(out);
    out.flush()?;
    let line = {
        let mut line = String::new();
        for _ in 0..8 {
            line.push(HORIZONTAL);
        }
        line
    };
    let mut keys = stdin.keys();
    keys.next().unwrap();
    write!(out, "{}", termion::cursor::Goto(1, 1))?;
    write!(out, "{}", line)?;
    out.flush()?;
    keys.next().unwrap();
    write!(out, "{}", termion::cursor::Goto(1, 1))?;
    write!(out, "{}", "你我他谁是谁\r\n")?;
    write!(out, "{}", "千里之行，始于足下\r\n")?;
    out.flush()?;
    keys.next().unwrap();

    write!(out, "{}", termion::cursor::Goto(2, 1))?;
    // write!(out, "\x1b[b;1,1,3,10$x")?;
    write!(out, "\x1b[2X")?;
    out.flush()?;
    keys.next().unwrap();
    write!(out, "{}", termion::cursor::Goto(1, 1))?;
    write!(out, "{}", HORIZONTAL)?;
    // write!(out,"{}", HORIZONTAL)?;
    // write!(out,"{}", termion::cursor::Goto(2,1))?;
    // write!(out,"{}", HORIZONTAL)?;
    // out.flush()?;
    // keys.next().unwrap();
    write!(out, "{}", termion::cursor::Goto(3, 1))?;
    write!(out, "{}", HORIZONTAL)?;
    // write!(out,"{}", termion::cursor::Goto(4,1))?;
    // write!(out,"{} ", HORIZONTAL)?;
    // out.flush()?;
    // keys.next().unwrap();
    write!(out, "{}", termion::cursor::Goto(5, 1))?;
    write!(out, "{}", HORIZONTAL)?;
    // write!(out,"{}", termion::cursor::Goto(6,1))?;
    // write!(out,"{} ", HORIZONTAL)?;
    // out.flush()?;
    // keys.next().unwrap();
    write!(out, "{}", termion::cursor::Goto(7, 1))?;
    write!(out, "{}", HORIZONTAL)?;
    // write!(out,"{}", termion::cursor::Goto(8,1))?;
    // write!(out,"{} ", HORIZONTAL)?;
    out.flush()?;
    keys.next().unwrap();

    Ok(())
}

// 测试上移
#[allow(dead_code)]
fn run6() -> Result<()> {
    let debuglog = File::create("window_debug.log")?;
    let _stderr_redirect = Redirect::stderr(debuglog)
        .map_err(|e| Error::RuntimeError(format!("Redirect stderr error {}", e)))?;
    let stdin = stdin();
    let mut terminal = Terminal::init()?;
    let (width, height) = termion::terminal_size()?;
    let flowarea = Rect {
        x: 1,
        y: 1,
        width,
        height: 4,
    };
    let mut flow = Flow::new(flowarea, 5000, true);
    flow.push_line(Line::fmt_raw("静言"));
    flow.push_line(Line::fmt_raw("┌───基本知识"));
    flow.push_line(Line::fmt_raw("│ 读书"));
    flow.push_line(Line::fmt_raw("│ 叫化"));
    terminal.render_widget(&mut flow, flowarea)?;
    terminal.flush(vec![flowarea])?;
    let mut keys = stdin.keys();
    keys.next().unwrap();

    flow.push_line(Line::fmt_raw("│ 道听"));
    terminal.render_widget(&mut flow, flowarea)?;
    terminal.flush(vec![flowarea])?;
    keys.next().unwrap();
    Ok(())
}

#[allow(dead_code)]
fn run5() -> Result<()> {
    let debuglog = File::create("window_debug.log")?;
    let _stderr_redirect = Redirect::stderr(debuglog)
        .map_err(|e| Error::RuntimeError(format!("Redirect stderr error {}", e)))?;

    let s = {
        let mut f = File::open("server.log")?;
        let mut s = String::new();
        f.read_to_string(&mut s)?;
        s
    };

    let stdin = stdin();
    let mut terminal = Terminal::init()?;
    // let mut buf = String::new();
    let (width, height) = termion::terminal_size()?;
    let flowarea = Rect {
        x: 1,
        y: 1,
        width,
        height: height - 1,
    };
    let mut flow = Flow::new(flowarea, 5000, true);

    for line in s.split('\n') {
        let mut line = line.to_owned();
        line.push('\n');
        flow.push_line(Line::fmt_raw(line));
    }

    terminal.render_widget(&mut flow, flowarea)?;
    // let (cursor_x, cursor_y) = cmdbar.cursor_pos(area, true);
    terminal.flush(vec![flowarea])?;
    terminal.set_cursor(1, height)?;

    for key in stdin.keys() {
        match key? {
            Key::Ctrl('q') => break,
            Key::Char(c) => {
                let mut cmd = String::new();
                cmd.push(c);
                cmd.push_str("\r\n");
                flow.push_line(Line::fmt_raw(cmd));
            }
            Key::Left => write!(terminal, "{}", termion::cursor::Left(1))?,
            _ => (),
        }
        terminal.render_widget(&mut flow, flowarea)?;
        terminal.flush(vec![flowarea])?;
        terminal.set_cursor(1, height)?;
    }
    Ok(())
}

// 测试block
#[allow(dead_code)]
fn run4() -> Result<()> {
    let debuglog = File::create("window_debug.log")?;
    let _stderr_redirect = Redirect::stderr(debuglog)
        .map_err(|e| Error::RuntimeError(format!("Redirect stderr error {}", e)))?;
    stderrlog::new()
        .module(module_path!())
        .verbosity(4)
        .timestamp(stderrlog::Timestamp::Millisecond)
        .init()
        .unwrap();

    let stdin = stdin();
    let mut terminal = Terminal::init()?;
    // let mut buf = String::new();
    let (width, height) = termion::terminal_size()?;
    let mut cmdbar = CmdBar::new('.', true, 200);
    let area = Rect {
        x: 1,
        y: height - 2,
        width,
        height: 3,
    };
    terminal.render_widget(&mut cmdbar, area)?;
    let (cursor_x, cursor_y) = cmdbar.cursor_pos(area, true);
    terminal.flush(vec![Rect {
        x: 1,
        y: 1,
        width,
        height,
    }])?;
    terminal.set_cursor(cursor_x, cursor_y)?;

    for key in stdin.keys() {
        match key? {
            Key::Ctrl('q') => break,
            Key::Char('\n') => {
                cmdbar.clear();
            }
            Key::Char(c) => {
                cmdbar.push_char(c);
            }
            Key::Left => write!(terminal, "{}", termion::cursor::Left(1))?,
            _ => (),
        }
        terminal.render_widget(&mut cmdbar, area)?;
        terminal.flush(vec![area])?;
        let (cursor_x, cursor_y) = cmdbar.cursor_pos(area, true);
        terminal.set_cursor(cursor_x, cursor_y)?;
    }
    Ok(())
}

// 测试buffer
#[allow(dead_code)]
fn run3() -> Result<()> {
    let debuglog = File::create("window_debug.log")?;
    let _stderr_redirect = Redirect::stderr(debuglog)
        .map_err(|e| Error::RuntimeError(format!("Redirect stderr error {}", e)))?;

    let stdin = stdin();
    let mut terminal = Terminal::init()?;
    // let mut buf = String::new();
    let (width, height) = termion::terminal_size()?;

    let mut buf = BufferVec::empty(Rect::new(3, 3, 7, 10));
    buf.set_style(*buf.area(), Style::default().bg(Color::Blue));

    let mut parser = Parser::default();
    parser.fill("\x1b[37;1m南京\x1b[44;1m是我的家乡\x1b[m");
    let mut line = Line::new(vec![]);
    while let Element::Span(span) = parser.next() {
        line.push_span(span);
    }

    let wl = line.wrap(buf.area().width as usize, true);
    for (i, l) in wl.0.into_iter().enumerate() {
        let (mut x, y) = (buf.area().left(), buf.area().top() + i as u16);
        for span in l.into_spans() {
            if let Some(pos) =
                buf.set_line_str(x, y, &span.content, buf.area().right(), span.style, true)
            {
                x = pos;
            }
        }
        // buf.set_line_str(3, 3, l buf.area.width, true)
    }

    // buf.set_line_str(3, 3, "南京是我的家乡", buf.area.width, Style::default(), true);

    for y in buf.area().top()..buf.area().bottom() {
        write!(terminal, "{}", termion::cursor::Goto(buf.area().left(), y))?;
        let mut to_skip: u16 = 0;
        let mut prev_style = None;
        for x in buf.area().left()..buf.area().right() {
            let c = buf.get(x, y);
            let cs = c.style();
            if let Some(ps) = prev_style {
                if cs != ps {
                    write_style(&mut terminal, cs)?;
                    prev_style = Some(cs);
                }
            } else {
                write_style(&mut terminal, cs)?;
                prev_style = Some(cs);
            }
            if to_skip == 0 {
                write!(&mut terminal, "{}", c.symbol.ch)?;
                to_skip = c.symbol.width.saturating_sub(1);
            } else {
                to_skip -= 1;
            }
        }
    }
    terminal.flush(vec![Rect {
        x: 1,
        y: 1,
        width,
        height,
    }])?;

    for key in stdin.keys() {
        match key? {
            Key::Ctrl('q') => break,
            Key::Char(c) => write!(terminal, "{}", c)?,
            Key::Left => write!(terminal, "{}", termion::cursor::Left(1))?,
            _ => (),
        }
        // cmdbar.draw(&mut terminal, true)?;
        terminal.flush(std::iter::once(Rect {
            x: 1,
            y: 1,
            width,
            height,
        }))?;
    }
    Ok(())
}

// 测试光标
#[allow(dead_code)]
fn run2() -> Result<()> {
    let debuglog = File::create("window_debug.log")?;
    let _stderr_redirect = Redirect::stderr(debuglog)
        .map_err(|e| Error::RuntimeError(format!("Redirect stderr error {}", e)))?;

    let stdin = stdin();
    let mut terminal = Terminal::init()?;
    // let mut buf = String::new();
    let (width, height) = termion::terminal_size()?;

    // let mut cmdbar = CmdBar::new();
    // cmdbar.draw(&mut terminal, true)?;
    // terminal.flush()?;

    write!(terminal, "{}", termion::cursor::BlinkingBar)?;
    write!(terminal, "{}", termion::cursor::Goto(1, 5))?;
    write!(terminal, "1234567890\r\n")?;
    // write!(terminal, "{}", termion::cursor::Goto(1, 6))?;
    write!(terminal, "123\tabc\r\n")?;
    write!(terminal, "{}", termion::cursor::Right(5))?;
    write!(terminal, "hello")?;
    write!(terminal, "{}", termion::cursor::Right(5))?;
    write!(terminal, "中国")?;
    // write!(terminal, "{}", termion::cursor::Save)?;
    let (cw, ch) = terminal.cursor_pos()?;
    write!(terminal, "\r{}", termion::cursor::Right(width - 1))?;
    write!(terminal, "x")?;
    // write!(terminal, "{}", termion::cursor::Restore)?;
    write!(terminal, "{}", termion::cursor::Goto(cw, ch))?;
    // write!(terminal, "\r")?;
    // write!(terminal, "abcde\r\n")?;
    terminal.flush(std::iter::once(Rect {
        x: 1,
        y: 1,
        width,
        height,
    }))?;

    for key in stdin.keys() {
        match key? {
            Key::Ctrl('q') => break,
            Key::Char(c) => write!(terminal, "{}", c)?,
            Key::Left => write!(terminal, "{}", termion::cursor::Left(1))?,
            _ => (),
        }
        // cmdbar.draw(&mut terminal, true)?;
        terminal.flush(std::iter::once(Rect {
            x: 1,
            y: 1,
            width,
            height,
        }))?;
    }
    Ok(())
}

// 测试ANSI日志
#[allow(dead_code)]
fn run1() -> Result<()> {
    let debuglog = File::create("window_debug.log")?;
    let _stderr_redirect = Redirect::stderr(debuglog)
        .map_err(|e| Error::RuntimeError(format!("Redirect stderr error {}", e)))?;

    let stdin = stdin();
    let mut terminal = Terminal::init()?;
    let (width, height) = terminal_size()?;
    let mut buf = String::new();
    // let mut lines = RawLines::unbounded();
    let mut lines = Vec::new();
    let mut curr_line = Line::new(vec![]);
    let mut spans = Parser::default();
    spans.fill(&read_file()?);
    while let Element::Span(span) = spans.next() {
        // eprintln!("span={:?}", span);
        // let s = if span.ended() {
        //     let mut s = span.content().to_owned();
        //     // s.push_str("\r\n");
        //     s
        // } else {
        //     span.content().to_owned()
        // };
        // lines.push_line(RawLine::owned(s));
        eprintln!("push span={}", span);
        curr_line.push_span(span);
        if curr_line.ended() {
            eprintln!("current line ended");
            lines.push(std::mem::replace(&mut curr_line, Line::new(vec![])));
        }
    }
    if !curr_line.spans().is_empty() {
        lines.push(curr_line);
    }
    render_lines(&mut terminal, &lines, &buf)?;

    for key in stdin.keys() {
        match key? {
            Key::Ctrl('q') => break,
            Key::Char('\n') => {
                let mut s = std::mem::replace(&mut buf, String::new());
                s.push_str("\r\n");
                // write!(terminal, "\r\n")?;
                // let line = RawLine::owned(s);
                // lines.push_line(line);
            }
            Key::Char(c) => {
                buf.push(c);
            }
            Key::Up => {
                write!(terminal, "{}", termion::scroll::Up(1))?;
                terminal.flush(std::iter::once(Rect {
                    x: 1,
                    y: 1,
                    width,
                    height,
                }))?;
            }
            Key::Down => {
                write!(terminal, "{}", termion::scroll::Down(1))?;
                terminal.flush(std::iter::once(Rect {
                    x: 1,
                    y: 1,
                    width,
                    height,
                }))?;
            }
            _ => (),
        }
        render_lines(&mut terminal, &lines, &buf)?;
    }
    Ok(())
}

#[allow(dead_code)]
fn render_lines<W: Write>(writer: &mut W, lines: &[Line], buf: &str) -> Result<()> {
    let (width, height) = terminal_size()?;
    for line in lines {
        eprintln!("line to wrap={:?}", line);
        // write!(writer, "{}", line.as_ref())?;
        let wls = line.wrap(width as usize, true);
        for wl in wls.0 {
            eprintln!("wrapped line={:?}", wl);
            for span in wl.spans() {
                eprintln!("span={:?}", span);
                write!(writer, "{}", span)?;
            }
            if !wl.ended() {
                write!(writer, "\r\n")?;
            }
        }
        // for span in &line.spans {
        //     write!(writer, "{}", span)?;
        // }
    }
    // 2 empty lines
    write!(writer, "\r\n{}", buf)?;
    writer.flush()?;
    Ok(())
}

#[allow(dead_code)]
fn render<W: Write>(writer: &mut W, lines: Vec<RawLine>, buf: &str) -> Result<()> {
    for line in lines {
        write!(writer, "{}", line.as_ref())?;
    }
    // 2 empty lines
    write!(writer, "\r\n{}", buf)?;
    writer.flush()?;
    Ok(())
}

#[allow(dead_code)]
fn read_file() -> Result<String> {
    let mut s = String::new();
    let mut f = File::open("server2.log")?;
    f.read_to_string(&mut s)?;
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_span() {
        let s = read_file().unwrap();
        let mut spans = AnsiParser::new();
        spans.fill(s);
        while let Some(span) = spans.next_span() {
            println!("span={}", span);
        }
    }
}
