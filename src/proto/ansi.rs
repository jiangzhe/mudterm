use crate::ui::style::{Color, Modifier, Style};

#[derive(Debug, Clone)]
pub struct ClearCells(pub u16);

impl std::fmt::Display for ClearCells {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "\x1b[{}X", self.0)
    }
}

/// 给定SGR字符串，应用相应的文本格式并返回
pub fn apply_sgr(mut style: Style, sgr: &str) -> Style {
    let mut n = 0;
    for c in sgr.chars() {
        match c {
            ';' => {
                style = apply_sgr_code(style, n);
                n = 0;
            }
            '0'..='9' => {
                n *= 10;
                n += (c as u8) - b'0';
            }
            other => {
                unreachable!("unreachable char '{}' in sgm sequence", other)
            }
        }
    }
    apply_sgr_code(style, n)
}

fn apply_sgr_code(mut style: Style, code: u8) -> Style {
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
        12 => {
            log::debug!("ESC[ 12 m => hide following chars(0x00 - 0x7F), not implemented");
            style
        }
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
        38 | 39 => {
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
        48 | 49 => {
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
