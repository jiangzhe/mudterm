// use tui::widgets::{Widget, Block};
// use tui::layout::Rect;
// use tui::buffer::Buffer;
// use tui::style::Style;
use std::collections::VecDeque;
use crate::ui::line::RawLine;
use crate::ui::span::ArcSpan;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;


/// 针对CJK宽字符实现的tui消息流
///
/// 向面板中添加的消息都将加入双向队列的尾部。
#[derive(Debug, Clone)]
pub struct MessageFlow {
    // 仅支持纵向滚动
    scroll: u16,
    // 关闭开启自动跟踪将通过调整scroll卯定在某一行
    auto_follow: bool,
    text: VecDeque<RawLine>,
    max_lines: u32,
    cjk: bool,
}

impl MessageFlow {
    pub fn new() -> Self {
        Self {
            scroll: 0,
            auto_follow: true,
            text: VecDeque::new(),
            max_lines: 5000,
            cjk: true,
        }
    }

    pub fn scroll(mut self, offset: u16) -> Self {
        self.scroll = offset;
        self
    }

    pub fn auto_follow(mut self, auto_follow: bool) -> Self {
        self.auto_follow = auto_follow;
        self
    }

    pub fn max_lines(mut self, max_lines: u32) -> Self {
        self.max_lines = max_lines;
        self
    }

    pub fn cjk(mut self, cjk: bool) -> Self {
        self.cjk = cjk;
        self
    }

    pub fn push_lines(&mut self, lines: Vec<Line>) {
        for line in lines {
            self.push_line(line);
        }
    }

    pub fn push_line(&mut self, line: Line) {
        if let Some(last_line) = self.text.back_mut() {
            if !last_line.ended() {
                last_line.spans.extend(line.spans);
                return;
            }
        }
        self.text.push_back(line);
        while self.text.len() > (self.max_lines as usize) {
            self.text.pop_front();
        }
    }

    pub fn all_spans(&self) -> Vec<ArcSpan> {
        self.text.iter().flat_map(|line| line.spans.iter().cloned()).collect()
    }
}

pub struct FlowBoard<'a, 'b>{
    flow: &'b MessageFlow,
    block: Block<'a>,
    style: Style,
}

impl<'a, 'b> FlowBoard<'a, 'b> {
    pub fn new(flow: &'b MessageFlow, block: Block<'a>, style: Style) -> Self {
        Self{flow, block, style}
    }
}

impl<'a, 'b> Widget for FlowBoard<'a, 'b> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let cjk = self.flow.cjk;
        let style = self.style;
        buf.set_style(area, style);
        let text_area = {
            let inner_area = self.block.inner(area);
            self.block.render(area, buf);
            inner_area
        };
        if text_area.height < 1 {
            return;
        }
        let mut y = if self.flow.auto_follow { text_area.height } else { text_area.height + self.flow.scroll };
        let mut lines = self.flow.text.iter();
        eprintln!("textwidth={}, textheight={}, textleft={}, texttop={}, y={}", text_area.width, text_area.height, text_area.left(), text_area.top(), y);
        'outer: while let Some(line) = lines.next_back() {
            eprintln!("line:");
            for span in &line.spans {
                eprint!("{}", span.content());
            }
            eprintln!();
            let wlines = line.wrap(text_area.width as usize, cjk);
            // eprintln!("wlines.len()={}", wlines.0.len()); 
            let mut wlines = wlines.0.iter();
            while let Some(line) = wlines.next_back() {
                let linewidth: usize = line.spans.iter().map(|s| s.width(cjk)).sum();
                eprintln!("line width={}", linewidth);
                y -= 1;
                if y < text_area.height {
                    let mut x = 0;
                    for span in &line.spans {
                        eprintln!("span='{}', bytes={:?}, color={:?}, x={}, width={}", span.content(), span.content().as_bytes(), span.style.fg, x, span.width(cjk));
                        for g in span.content().graphemes(true) {
                            let g = if g.is_empty() { " " } else { g };
                            buf.get_mut(text_area.left() + x, text_area.top() + y - self.flow.scroll)
                                .set_symbol(g)
                                .set_style(style.patch(span.style));
                            x += if cjk { g.width_cjk() } else { g.width() } as u16;
                        }
                        if span.ended {
                            buf.get_mut(text_area.left() + x, text_area.top() + y - self.flow.scroll)
                                .set_symbol("\n")
                                .set_style(style.patch(span.style));
                        }
                    }
                }
                if y == 0 {
                    break 'outer;
                }
            }
        }
    }
}