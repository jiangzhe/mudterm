use crate::ui::line::Line;
use crate::ui::span::Span;
use unicode_width::UnicodeWidthChar;

pub trait DisplayWidthMaybeZero {
    fn display_width(&self, cjk: bool) -> usize;
}

impl DisplayWidthMaybeZero for char {
    fn display_width(&self, cjk: bool) -> usize {
        if cjk {
            self.width_cjk().unwrap_or(0)
        } else {
            self.width().unwrap_or(0)
        }
    }
}

pub trait AppendWidthTab8 {
    const TAB_SPACES: usize = 8;

    // given previous width and cjk flag, returns next width
    fn append_width(&self, prev_width: usize, cjk: bool) -> usize;
}

impl AppendWidthTab8 for char {
    fn append_width(&self, prev_width: usize, cjk: bool) -> usize {
        if *self == '\t' {
            prev_width / Self::TAB_SPACES * Self::TAB_SPACES + Self::TAB_SPACES
        } else {
            prev_width
                + if cjk {
                    self.width_cjk().unwrap_or(0)
                } else {
                    self.width().unwrap_or(0)
                }
        }
    }
}

impl AppendWidthTab8 for str {
    fn append_width(&self, prev_width: usize, cjk: bool) -> usize {
        self.chars().fold(prev_width, |w, c| {
            if c == '\t' {
                w / Self::TAB_SPACES * Self::TAB_SPACES + Self::TAB_SPACES
            } else {
                w + if cjk {
                    c.width_cjk().unwrap_or(0)
                } else {
                    c.width_cjk().unwrap_or(0)
                }
            }
        })
    }
}

impl AppendWidthTab8 for Span {
    fn append_width(&self, prev_width: usize, cjk: bool) -> usize {
        self.content.append_width(prev_width, cjk)
    }
}

impl AppendWidthTab8 for Vec<Span> {
    fn append_width(&self, prev_width: usize, cjk: bool) -> usize {
        self.iter().fold(prev_width, |w, s| s.append_width(w, cjk))
    }
}

impl AppendWidthTab8 for Line {
    fn append_width(&self, prev_width: usize, cjk: bool) -> usize {
        self.spans.append_width(prev_width, cjk)
    }
}

#[cfg(test)]
mod tests {
    use super::AppendWidthTab8;
    use crate::ui::line::Line;
    use crate::ui::span::Span;
    use crate::ui::style::Style;

    #[test]
    fn test_append_width() {
        let s = "123";
        assert_eq!(3, s.append_width(0, true));
        let s = "123\t";
        assert_eq!(8, s.append_width(0, true));
        let s = "123\t456";
        assert_eq!(11, s.append_width(0, true));
        let s = Line::fmt_raw("hello");
        assert_eq!(5, s.append_width(0, true));
        let s = Line::new(vec![
            Span::new("hello", Style::default()),
            Span::new("\tworld", Style::default()),
        ]);
        assert_eq!(13, s.append_width(0, true));
    }
}
