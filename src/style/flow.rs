use tui::widgets::{Widget, Block};
use tui::layout::Rect;
use tui::buffer::Buffer;
use tui::style::Style;
use tui::text::StyledGrapheme;
use std::collections::VecDeque;
use std::iter;
use crate::style::line::Line;
// padding when rendering
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};


#[derive(Debug, Clone)]
/// 针对CJK宽字符实现的tui消息流
///
/// 向面板中添加的消息都将加入双向队列的尾部。
/// 
pub struct MessageFlow<'a> {
    block: Option<Block<'a>>,
    style: Style,
    // only support vertical scrolling
    scroll: u16,
    // 关闭开启自动跟踪将通过调整scroll卯定在某一行
    auto_follow: bool,
    buf: Vec<u8>,
    text: VecDeque<Line>,
    max_width: u16,
    max_lines: u32,
}

impl<'a> MessageFlow<'a> {
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn scroll(mut self, offset: u16) -> Self {
        self.scroll = offset;
        self
    }

    pub fn auto_follow(mut self, auto_follow: bool) -> Self {
        self.auto_follow = auto_follow;
        self
    }

    pub fn max_width(mut self, max_width: u16) -> Self {
        self.max_width = max_width;
        self
    }

    pub fn max_lines(mut self, max_lines: u32) -> Self {
        self.max_lines = max_lines;
        self
    }
}

// impl<'a> Widget for MessageFlow<'a> {
//     fn render(mut self, area: Rect, buf: &mut Buffer) {
//         buf.set_style(area, self.style);
//         let text_area = match self.block.take() {
//             Some(b) => {
//                 let inner_area = b.inner(area);
//                 b.render(area, buf);
//                 inner_area
//             }
//             None => area,
//         };
//         if text_area.height < 1 {
//             return;
//         }
//         let style = self.style;
//         let mut styled = self.text.iter().flat_map(|line| {
//             line.spans.iter()
//                 .flat_map(|span| span.styled_graphemes(style))
//                 .chain(iter::once(StyledGrapheme{
//                     symbol: "\n",
//                     style,
//                 }))
//         });
//         let line_composer: Box<dyn LineComposer> = todo!();
//         let mut y = 0;
//         while let Some((curr_line, curr_line_width)) = line_composer.next_line() {
//             if y >= self.scroll {
//                 let mut x = 0;
//                 for StyledGrapheme { symbol, style } in curr_line {
//                     buf.get_mut(text_area.left() + x, text_area.top() + y - self.scroll)
//                         .set_symbol(if symbol.is_empty() {
//                             " "
//                         } else {
//                             symbol
//                         })
//                         .set_style(*style);
//                     x += symbol.width_cjk() as u16;
//                 }
//             }
//             y += 1;
//             if y >= text_area.height + self.scroll {
//                 break;
//             }
//         }
//     }
// }


// todo
pub trait LineComposer<'a> {
    fn next_line(&mut self) -> Option<(&[StyledGrapheme<'a>], u16)>;
}