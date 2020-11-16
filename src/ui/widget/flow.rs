use crate::ui::ansi::AnsiParser;
use crate::ui::line::{Line, RawLine, WrapLine};
use crate::ui::layout::Rect;
use crate::ui::widget::Widget;
use crate::ui::buffer::Buffer;
use crate::error::Result;
use std::collections::VecDeque;
use std::collections::vec_deque::Iter;

pub struct Flow {
    area: Rect,
    max_lines: usize,
    raw: VecDeque<RawLine>,
    display: VecDeque<WrapLine>,
    parser: AnsiParser,
    // ready: usize,
    // partial_ready: bool,
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
        self.parse_ansi_line(line.content());
        // 原ansi字符序列
        if let Some(last_line) = self.raw.back_mut() {
            if !last_line.ended() {
                last_line.push_line(line);
                // self.partial_ready = true;
                return;
            }
        }
        self.raw.push_back(line);
        // self.ready += 1;
        while self.raw.len() > self.max_lines {
            self.raw.pop_front();
        }
    }

    fn parse_ansi_line(&mut self, line: impl AsRef<str>) {
        self.parser.fill(line.as_ref());
        while let Some(span) = self.parser.next_span() {
            let last_line = self.display.back_mut().unwrap();
            if !last_line.ended() {
                last_line.push_span(span, self.area.width as usize, self.cjk);
            } else {
                let line = Line::single(span);
                let wl = line.wrap(self.area.width as usize, self.cjk);
                self.display.push_back(wl);
            }
        }
        while self.display.len() > self.max_lines {
            self.display.pop_front();
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
            self.parse_ansi_line(line);
        }
    }

    pub fn display_lines(&self) -> Iter<'_, WrapLine> {
        self.display.iter()
    }

    pub fn line_by_y(&self, offset: usize) -> Option<RawLine> {
        self.raw.iter().rev().nth(offset).cloned()
    }
}

pub struct FlowLines<'a> {
    lines: Iter<'a, WrapLine>,
}

impl<'a> Widget for FlowLines<'a> {

    fn refresh_buffer<B: Buffer>(&mut self, buf: &mut B, cjk: bool) -> Result<()> {
        let mut y = buf.area().top();
        while let Some(wl) = self.lines.next() {
            for l in &wl.0 {
                let mut x = buf.area().left();
                for span in l.spans {
                    if let Some(pos) =
                        buf.set_line_str(x, y, &span.content, buf.area().right(), span.style, cjk)
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