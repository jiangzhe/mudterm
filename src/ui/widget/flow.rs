use crate::error::Result;
use crate::ui::ansi::AnsiParser;
use crate::ui::buffer::Buffer;
use crate::ui::layout::Rect;
use crate::ui::line::{Line, RawLine, WrapLine};
use crate::ui::widget::Widget;
use std::collections::vec_deque::Iter;
use std::collections::VecDeque;

pub struct Flow {
    area: Rect,
    max_lines: usize,
    raw: VecDeque<RawLine>,
    display: VecDeque<WrapLine>,
    parser: AnsiParser,
    cjk: bool,
}

impl Flow {
    pub fn new(area: Rect, max_lines: usize, cjk: bool) -> Self {
        debug_assert!(max_lines >= area.height as usize);
        let mut flow = Self {
            area,
            max_lines,
            raw: VecDeque::new(),
            display: VecDeque::new(),
            parser: AnsiParser::new(),
            cjk,
        };

        for _ in 0..area.height {
            flow.push_line(RawLine::owned("\r\n".to_owned()));
        }

        flow
    }

    pub fn push_line(&mut self, line: RawLine) {
        // 解析序列
        parse_ansi_line(
            &mut self.parser,
            &mut self.display,
            line.content(),
            self.area.width as usize,
            self.cjk,
            self.area.height as usize,
        );
        // 原ansi字符序列
        if let Some(last_line) = self.raw.back_mut() {
            if !last_line.ended() {
                last_line.push_line(line);
                return;
            }
        }
        self.raw.push_back(line);
        // self.ready += 1;
        while self.raw.len() > self.max_lines {
            self.raw.pop_front();
        }
    }

    pub fn push_lines(&mut self, lines: impl IntoIterator<Item = RawLine>) {
        for line in lines {
            self.push_line(line);
        }
    }

    pub fn reshape(&mut self, area: Rect) {
        self.area = area;
        self.parser = AnsiParser::new();
        self.display.clear();
        for line in self.raw.iter().rev().take(self.area.height as usize).rev() {
            parse_ansi_line(
                &mut self.parser,
                &mut self.display,
                line,
                self.area.width as usize,
                self.cjk,
                self.area.height as usize,
            );
        }
    }

    pub fn display_lines(&self) -> Iter<'_, WrapLine> {
        self.display.iter()
    }

    pub fn line_by_y(&self, offset: usize) -> Option<RawLine> {
        self.raw.iter().rev().nth(offset).cloned()
    }
}

/// 克服Rust borrowchecker的结构化限制
fn parse_ansi_line(
    parser: &mut AnsiParser,
    display: &mut VecDeque<WrapLine>,
    line: impl AsRef<str>,
    width: usize,
    cjk: bool,
    height: usize,
) {
    parser.fill(line.as_ref());
    while let Some(span) = parser.next_span() {
        if let Some(last_line) = display.back_mut() {
            if !last_line.ended() {
                last_line.push_span(span, width, cjk);
            } else {
                let line = Line::single(span);
                let wl = line.wrap(width, cjk);
                display.push_back(wl);
            }
        } else {
            let line = Line::single(span);
            let wl = line.wrap(width, cjk);
            display.push_back(wl);
        }
    }
    let mut len: usize = display.iter().map(|wl| wl.0.len()).sum();
    if len > height {
        'outer: loop {
            let mut head = display.pop_front().unwrap();
            if head.0.len() == 1 {
                len -= 1;
                if len == height {
                    break 'outer;
                }
            } else {
                while let Some(_) = head.0.pop() {
                    len -= 1;
                    if len == height {
                        break 'outer;
                    }
                }
            }
        }
    }
}

impl Widget for Flow {
    fn refresh_buffer<B: Buffer>(&mut self, buf: &mut B) -> Result<()> {
        let mut y = buf.area().top();
        // eprintln!("flow area {:?}", buf.area());
        // eprintln!("display wl size {}", self.display.len());
        // eprintln!(
        //     "display line size {}",
        //     self.display.iter().map(|wl| wl.0.len()).sum::<usize>()
        // );
        for wl in &self.display {
            for l in &wl.0 {
                let mut x = buf.area().left();
                for span in &l.spans {
                    if let Some(pos) =
                        buf.set_line_str(x, y, &span.content, buf.area().right(), span.style, self.cjk)
                    {
                        x = pos;
                    }
                }
                y += 1;
            }
        }
        Ok(())
    }
}
