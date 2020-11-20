use crate::error::Result;
use crate::ui::buffer::Buffer;
use crate::ui::layout::Rect;
use crate::ui::line::{Line, WrapLine};
use crate::ui::widget::Widget;
use std::collections::vec_deque::Iter;
use std::collections::VecDeque;

pub struct Flow {
    area: Rect,
    max_lines: usize,
    history: VecDeque<Line>,
    display: VecDeque<WrapLine>,
    cjk: bool,
}

impl Flow {
    pub fn new(area: Rect, mut max_lines: usize, cjk: bool) -> Self {
        if max_lines < area.height as usize {
            max_lines = area.height as usize;
        }
        let mut flow = Self {
            area,
            max_lines,
            history: VecDeque::new(),
            display: VecDeque::new(),
            cjk,
        };

        for _ in 0..area.height {
            flow.push_line(Line::fmt_raw(""));
        }

        flow
    }

    /// 输入必须为单行
    fn push_history(&mut self, line: Line) {
        if let Some(last_line) = self.history.back_mut() {
            if !last_line.ended() {
                last_line.push_line(line);
                return;
            }
        }
        self.history.push_back(line);
        while self.history.len() > self.max_lines {
            self.history.pop_front();
        }
    }

    pub fn push_line(&mut self, line: Line) {
        self.push_history(line.clone());
        for span in line.into_spans() {
            if let Some(last_line) = self.display.back_mut() {
                if !last_line.ended() {
                    last_line.push_span(span, self.area.width as usize, self.cjk);
                } else {
                    let line = Line::single(span);
                    let wl = line.wrap(self.area.width as usize, self.cjk);
                    self.display.push_back(wl);
                }
            } else {
                let line = Line::single(span);
                let wl = line.wrap(self.area.width as usize, self.cjk);
                self.display.push_back(wl);
            }
        }
        let mut len: usize = self.display.iter().map(|wl| wl.0.len()).sum();
        if len > self.area.height as usize {
            'outer: loop {
                let mut head = self.display.pop_front().unwrap();
                if head.0.len() == 1 {
                    len -= 1;
                    if len == self.area.height as usize {
                        break 'outer;
                    }
                } else {
                    while let Some(_) = head.0.pop() {
                        len -= 1;
                        if len == self.area.height as usize {
                            break 'outer;
                        }
                    }
                }
            }
        }
    }

    pub fn push_lines(&mut self, lines: impl IntoIterator<Item = Line>) {
        for line in lines {
            self.push_line(line);
        }
    }

    pub fn reshape(&mut self, area: Rect) {
        // 更新区域
        self.area = area;
        // 更新最大行数
        let height = area.height as usize;
        if self.max_lines < height {
            self.max_lines = height;
        }
        // 重新填充历史文本
        let lines = if self.history.len() < height {
            let mut lines = Vec::with_capacity(height);
            for _ in 0..height - self.history.len() {
                lines.push(Line::fmt_raw(""));
            }
            lines.extend(self.history.drain(..));
            lines
        } else {
            self.history.drain(self.history.len() - height..).collect()
        };
        self.display.clear();
        for line in lines {
            self.push_line(line);
        }
    }

    pub fn display_lines(&self) -> Iter<'_, WrapLine> {
        self.display.iter()
    }
}

impl Widget for Flow {
    fn refresh_buffer<B: Buffer>(&mut self, buf: &mut B) -> Result<()> {
        let mut y = buf.area().top();
        for wl in &self.display {
            for l in &wl.0 {
                let mut x = buf.area().left();
                for span in l.spans() {
                    if let Some(pos) = buf.set_line_str(
                        x,
                        y,
                        &span.content,
                        buf.area().right(),
                        span.style,
                        self.cjk,
                    ) {
                        x = pos;
                    }
                }
                y += 1;
            }
        }
        Ok(())
    }
}
