use crate::error::Result;
use crate::ui::buffer::{Buffer, Symbol};
use crate::ui::layout::Rect;
use crate::ui::symbol::*;
use crate::ui::widget::Widget;

pub fn inner_area(area: Rect, cjk: bool) -> Rect {
    let width = if cjk { 2 } else { 1 };
    Rect {
        x: area.x + width,
        y: area.y + 1,
        width: area.width - 2 * width,
        height: area.height - 2,
    }
}

pub enum Border {
    Rounded,
    Square,
}

impl Widget for Border {
    fn refresh_buffer<B: Buffer>(&mut self, buf: &mut B, cjk: bool) -> Result<()> {
        if buf.area().height < 2 {
            return Ok(());
        }

        let (top_left, top_right, bottom_left, bottom_right) = match self {
            Self::Rounded => (
                ROUNDED_TOP_LEFT,
                ROUNDED_TOP_RIGHT,
                ROUNDED_BOTTOM_LEFT,
                ROUNDED_BOTTOM_RIGHT,
            ),
            Self::Square => (TOP_LEFT, TOP_RIGHT, BOTTOM_LEFT, BOTTOM_RIGHT),
        };
        let width: u16 = if cjk { 2 } else { 1 };
        // right() - 1 to handle both even and odd width
        for y in vec![buf.area().top(), buf.area().bottom() - 1] {
            for x in (buf.area().left() + width..buf.area().right() - 1).step_by(width as usize) {
                buf.get_mut(x, y).set_symbol(Symbol {
                    ch: HORIZONTAL,
                    width,
                    exists: false,
                });
            }
        }
        for x in vec![buf.area().left(), buf.area().right() - width] {
            for y in buf.area().top() + 1..buf.area().bottom() - 1 {
                buf.get_mut(x, y).set_symbol(Symbol {
                    ch: VERTICAL,
                    width,
                    exists: false,
                });
            }
        }
        buf.get_mut(buf.area().left(), buf.area().top())
            .set_symbol(Symbol {
                ch: top_left,
                width,
                exists: false,
            });
        buf.get_mut(buf.area().right() - width, buf.area().top())
            .set_symbol(Symbol {
                ch: top_right,
                width,
                exists: false,
            });
        buf.get_mut(buf.area().left(), buf.area().bottom() - 1)
            .set_symbol(Symbol {
                ch: bottom_left,
                width,
                exists: false,
            });
        buf.get_mut(buf.area().right() - width, buf.area().bottom() - 1)
            .set_symbol(Symbol {
                ch: bottom_right,
                width,
                exists: false,
            });

        Ok(())
    }
}
