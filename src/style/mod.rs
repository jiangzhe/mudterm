pub mod reflector;
pub mod flow;
pub mod line;
pub mod ansi;

use tui::style::{Color, Modifier, Style};
use tui::text::Span;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
use reflector::CJKStringAdapter;

#[derive(Debug, Clone, PartialEq)]
pub struct StyledLine {
    pub spans: Vec<Span<'static>>,
    pub orig: String,
    pub ended: bool,
}

impl StyledLine {
    pub fn empty() -> Self {
        Self {
            spans: Vec::new(),
            orig: String::new(),
            ended: false,
        }
    }

    pub fn raw(line: String) -> Self {
        Self {
            spans: vec![Span::raw(line.to_owned())],
            orig: line,
            ended: true,
        }
    }

    pub fn err(s: String) -> StyledLine {
        let style = Style::default().add_modifier(Modifier::REVERSED);
        StyledLine {
            spans: vec![Span::styled(s.to_owned(), style)],
            orig: s,
            ended: true,
        }
    }

    pub fn note(s: String) -> StyledLine {
        let style = Style::default().fg(Color::LightBlue);
        StyledLine {
            spans: vec![Span::styled(s.to_owned(), style)],
            orig: s,
            ended: true,
        }
    }

    pub fn extract(&mut self, adapter: &mut CJKStringAdapter, style: Style, ended: bool) {
        let (orig, adapted) = adapter.drain_origin_and_extended();
        self.orig.push_str(&orig);
        self.spans.push(Span::styled(adapted, style));
        self.ended = ended;
    }

    pub fn split_with_max_width(&self, max_width: u16) -> Vec<Vec<Span<'static>>> {
        if self.orig.width_cjk() <= max_width as usize {
            return vec![self.spans.clone()];
        }
        let mut lines = Vec::new();
        let mut curr_width = 0;
        let mut curr_line = Vec::new();
        for span in &self.spans {
            // here use width because already filled with space to make width
            // and width_cjk identical
            if span.content.width() + curr_width <= max_width as usize {
                curr_line.push(span.clone());
                curr_width += span.content.width();
            } else {
                let new_style = span.style;
                let mut new_content = String::new();
                for c in span.content.chars() {
                    let cw = c.width().unwrap_or(0);
                    if curr_width + cw <= max_width as usize {
                        new_content.push(c);
                        curr_width += cw;
                    } else {
                        // exceeds max width
                        let new_span = Span::styled(
                            std::mem::replace(&mut new_content, String::new()),
                            new_style,
                        );
                        curr_line.push(new_span);
                        lines.push(std::mem::replace(&mut curr_line, Vec::new()));
                        // we need to push current char to new content
                        new_content.push(c);
                        curr_width = cw;
                    }
                }
                // concat last span to curr_line
                if !new_content.is_empty() {
                    let new_span = Span::styled(new_content, new_style);
                    curr_line.push(new_span);
                }
            }
        }
        if !curr_line.is_empty() {
            lines.push(curr_line);
        }
        lines
    }
}


/// todo: check 37, 90, 97 color matching
fn apply_ansi_sgr(mut style: Style, code: u8) -> Style {
    match code {
        0 => Style::default(),
        1 => style.add_modifier(Modifier::BOLD),
        2 => style.add_modifier(Modifier::DIM),
        3 => style.add_modifier(Modifier::ITALIC),
        4 => style.add_modifier(Modifier::UNDERLINED),
        5 => style.add_modifier(Modifier::SLOW_BLINK),
        6 => style.add_modifier(Modifier::RAPID_BLINK),
        7 => style.add_modifier(Modifier::REVERSED),
        8 => style.add_modifier(Modifier::HIDDEN),
        9 => style.add_modifier(Modifier::CROSSED_OUT),
        21 => style.remove_modifier(Modifier::BOLD),
        22 => style.remove_modifier(Modifier::DIM),
        23 => style.remove_modifier(Modifier::ITALIC),
        24 => style.remove_modifier(Modifier::UNDERLINED),
        25 => style
            .remove_modifier(Modifier::SLOW_BLINK)
            .remove_modifier(Modifier::RAPID_BLINK),
        27 => style.remove_modifier(Modifier::REVERSED),
        28 => style.remove_modifier(Modifier::HIDDEN),
        29 => style.remove_modifier(Modifier::CROSSED_OUT),
        // frontend color
        30 => style.fg(Color::Black),
        31 => style.fg(Color::Red),
        32 => style.fg(Color::Green),
        33 => style.fg(Color::Yellow),
        34 => style.fg(Color::Blue),
        35 => style.fg(Color::Magenta),
        36 => style.fg(Color::Cyan),
        37 => style.fg(Color::Gray),
        38 => unimplemented!(),
        39 => {
            style.fg = None;
            style
        }
        90 => style.fg(Color::DarkGray),
        91 => style.fg(Color::LightRed),
        92 => style.fg(Color::LightGreen),
        93 => style.fg(Color::LightYellow),
        94 => style.fg(Color::LightBlue),
        95 => style.fg(Color::LightMagenta),
        96 => style.fg(Color::LightCyan),
        97 => style.fg(Color::White),
        // backend color
        40 => style.bg(Color::Black),
        41 => style.bg(Color::Red),
        42 => style.bg(Color::Green),
        43 => style.bg(Color::Yellow),
        44 => style.bg(Color::Blue),
        45 => style.bg(Color::Magenta),
        46 => style.bg(Color::Cyan),
        47 => style.bg(Color::Gray),
        48 => unimplemented!(),
        49 => {
            style.bg = None;
            style
        }
        100 => style.bg(Color::DarkGray),
        101 => style.bg(Color::LightRed),
        102 => style.bg(Color::LightGreen),
        103 => style.bg(Color::LightYellow),
        104 => style.bg(Color::LightBlue),
        105 => style.bg(Color::LightMagenta),
        106 => style.bg(Color::LightCyan),
        107 => style.bg(Color::White),
        _ => panic!("unknown SGR argument {}", code),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_character_width() {
        let s0 = "│";
        println!(
            "s0\"{}\" width={}, width_cjk={}",
            s0,
            s0.width(),
            s0.width_cjk()
        );
        let s1 = "　";
        println!(
            "s1\"{}\" width={}, width_cjk={}",
            s1,
            s1.width(),
            s1.width_cjk()
        );
    }
}
