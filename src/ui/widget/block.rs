use crate::error::Result;
use crate::ui::buffer::{Buffer, Symbol};
use crate::ui::layout::Rect;
use crate::ui::style::Style;
use crate::ui::symbol::*;
use crate::ui::widget::Widget;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Border {
    Rounded,
    Square,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub border: Border,
    pub style: Style,
    pub cjk: bool,
}

impl Default for Block {
    fn default() -> Self {
        Self {
            border: Border::Rounded,
            style: Style::default(),
            cjk: true,
        }
    }
}

impl Block {
    pub fn border(mut self, border: Border) -> Self {
        self.border = border;
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn cjk(mut self, cjk: bool) -> Self {
        self.cjk = cjk;
        self
    }

    pub fn inner_area(&self, area: Rect) -> Rect {
        let cw = if self.cjk { 2 } else { 1 };
        let width = if area.width & 1 == 1 && self.cjk {
            // 由于边框字符宽度为2，如果区域宽度不为偶数，需要舍弃最后一列
            area.width - 2 * cw - 1
        } else {
            area.width - 2 * cw
        };
        Rect {
            x: area.x + cw,
            y: area.y + 1,
            width,
            height: area.height - 2,
        }
    }

    pub fn outer_area(&self, area: Rect) -> Rect {
        let width = if area.width & 1 == 1 && self.cjk {
            area.width - 1
        } else {
            area.width
        };
        Rect {
            x: area.x,
            y: area.y,
            width,
            height: area.height,
        }
    }
}

impl Widget for Block {
    fn refresh_buffer<B: Buffer>(&mut self, buf: &mut B) -> Result<()> {
        if buf.area().height < 2 {
            return Ok(());
        }
        let area = self.outer_area(*buf.area());

        let (top_left, top_right, bottom_left, bottom_right) = match self.border {
            Border::Rounded => (
                ROUNDED_TOP_LEFT,
                ROUNDED_TOP_RIGHT,
                ROUNDED_BOTTOM_LEFT,
                ROUNDED_BOTTOM_RIGHT,
            ),
            Border::Square => (TOP_LEFT, TOP_RIGHT, BOTTOM_LEFT, BOTTOM_RIGHT),
        };
        let sw: u16 = if self.cjk { 2 } else { 1 };
        // right() - 1 to handle both even and odd width
        for y in vec![area.top(), area.bottom() - 1] {
            for x in (area.left() + sw..area.right() - sw).step_by(sw as usize) {
                buf.get_mut(x, y).set_style(self.style).set_symbol(Symbol {
                    ch: HORIZONTAL,
                    width: sw,
                    exists: false,
                });
            }
        }
        // right() - 1 to handle both even and odd width
        for x in vec![area.left(), area.right() - sw] {
            for y in area.top() + 1..area.bottom() - 1 {
                buf.get_mut(x, y).set_style(self.style).set_symbol(Symbol {
                    ch: VERTICAL,
                    width: sw,
                    exists: false,
                });
            }
        }
        buf.get_mut(area.left(), area.top())
            .set_style(self.style)
            .set_symbol(Symbol {
                ch: top_left,
                width: sw,
                exists: false,
            });
        buf.get_mut(area.right() - sw, area.top())
            .set_style(self.style)
            .set_symbol(Symbol {
                ch: top_right,
                width: sw,
                exists: false,
            });
        buf.get_mut(area.left(), area.bottom() - 1)
            .set_style(self.style)
            .set_symbol(Symbol {
                ch: bottom_left,
                width: sw,
                exists: false,
            });
        buf.get_mut(area.right() - sw, area.bottom() - 1)
            .set_style(self.style)
            .set_symbol(Symbol {
                ch: bottom_right,
                width: sw,
                exists: false,
            });
        Ok(())
    }
}
