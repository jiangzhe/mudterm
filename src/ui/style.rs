use bitflags::bitflags;

#[derive(Debug, Clone, Copy)]
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub add_modifier: Modifier,
    pub sub_modifier: Modifier,
}

impl PartialEq for Style {
    fn eq(&self, other: &Self) -> bool {
        let p0 = Style::default().patch(*self);
        let p1 = Style::default().patch(*other);
        p0.fg == p1.fg && p0.bg == p1.bg && 
        p0.add_modifier == p1.add_modifier &&
        p0.sub_modifier == p1.sub_modifier
    }
}

impl Default for Style {
    fn default() -> Self {
        Self {
            fg: None,
            bg: None,
            add_modifier: Modifier::empty(),
            sub_modifier: Modifier::empty(),
        }
    }
}

impl std::fmt::Display for Style {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.fg.is_none()
            && self.bg.is_none()
            && self.add_modifier.is_empty()
            && self.sub_modifier.is_empty()
        {
            write!(f, "\x1b[m")?;
            return Ok(());
        }
        write!(f, "\x1b[")?;
        let mut require_colon = false;
        if let Some(fg) = self.fg {
            match fg {
                Color::Reset => write!(f, "39")?,
                Color::Black => write!(f, "30")?,
                Color::Red => write!(f, "31")?,
                Color::Green => write!(f, "32")?,
                Color::Yellow => write!(f, "33")?,
                Color::Blue => write!(f, "34")?,
                Color::Magenta => write!(f, "35")?,
                Color::Cyan => write!(f, "36")?,
                Color::Gray => write!(f, "37")?,
                Color::DarkGray => write!(f, "90")?,
                Color::LightRed => write!(f, "91")?,
                Color::LightGreen => write!(f, "92")?,
                Color::LightYellow => write!(f, "93")?,
                Color::LightBlue => write!(f, "94")?,
                Color::LightMagenta => write!(f, "95")?,
                Color::LightCyan => write!(f, "96")?,
                Color::White => write!(f, "97")?,
            }
            require_colon = true;
        }
        if let Some(bg) = self.bg {
            if require_colon {
                write!(f, ";")?;
            }
            match bg {
                Color::Reset => write!(f, "49")?,
                Color::Black => write!(f, "40")?,
                Color::Red => write!(f, "41")?,
                Color::Green => write!(f, "42")?,
                Color::Yellow => write!(f, "43")?,
                Color::Blue => write!(f, "44")?,
                Color::Magenta => write!(f, "45")?,
                Color::Cyan => write!(f, "46")?,
                Color::Gray => write!(f, "47")?,
                Color::DarkGray => write!(f, "100")?,
                Color::LightRed => write!(f, "101")?,
                Color::LightGreen => write!(f, "102")?,
                Color::LightYellow => write!(f, "103")?,
                Color::LightBlue => write!(f, "104")?,
                Color::LightMagenta => write!(f, "105")?,
                Color::LightCyan => write!(f, "106")?,
                Color::White => write!(f, "107")?,
            }
            require_colon = true;
        }
        if !self.add_modifier.is_empty() {
            if require_colon {
                write!(f, ";")?;
            }
            if self.add_modifier.contains(Modifier::BOLD) {
                write!(f, "1")?;
            }
            if self.add_modifier.contains(Modifier::DIM) {
                write!(f, "2")?;
            }
            if self.add_modifier.contains(Modifier::ITALIC) {
                write!(f, "3")?;
            }
            if self.add_modifier.contains(Modifier::UNDERLINED) {
                write!(f, "4")?;
            }
            if self.add_modifier.contains(Modifier::SLOW_BLINK) {
                write!(f, "5")?;
            }
            if self.add_modifier.contains(Modifier::RAPID_BLINK) {
                write!(f, "6")?;
            }
            if self.add_modifier.contains(Modifier::REVERSED) {
                write!(f, "7")?;
            }
            if self.add_modifier.contains(Modifier::HIDDEN) {
                write!(f, "8")?;
            }
            if self.add_modifier.contains(Modifier::CROSSED_OUT) {
                write!(f, "9")?;
            }
            require_colon = true;
        }
        if !self.sub_modifier.is_empty() {
            if require_colon {
                write!(f, ";")?;
            }
            if self.sub_modifier.contains(Modifier::BOLD) {
                write!(f, "21")?;
            }
            if self.sub_modifier.contains(Modifier::DIM) {
                write!(f, "22")?;
            }
            if self.sub_modifier.contains(Modifier::ITALIC) {
                write!(f, "23")?;
            }
            if self.sub_modifier.contains(Modifier::UNDERLINED) {
                write!(f, "24")?;
            }
            if self.sub_modifier.contains(Modifier::SLOW_BLINK) {
                write!(f, "25")?;
            }
            if self.sub_modifier.contains(Modifier::RAPID_BLINK) {
                write!(f, "25")?;
            }
            if self.sub_modifier.contains(Modifier::REVERSED) {
                write!(f, "27")?;
            }
            if self.sub_modifier.contains(Modifier::HIDDEN) {
                write!(f, "28")?;
            }
            if self.sub_modifier.contains(Modifier::CROSSED_OUT) {
                write!(f, "29")?;
            }
        }
        write!(f, "m")?;

        Ok(())
    }
}

impl Style {
    pub fn fg(mut self, color: Color) -> Style {
        self.fg = Some(color);
        self
    }

    pub fn bg(mut self, color: Color) -> Style {
        self.bg = Some(color);
        self
    }

    pub fn add_modifier(mut self, modifier: Modifier) -> Style {
        self.sub_modifier.remove(modifier);
        self.add_modifier.insert(modifier);
        self
    }

    pub fn remove_modifier(mut self, modifier: Modifier) -> Style {
        self.add_modifier.remove(modifier);
        self.sub_modifier.insert(modifier);
        self
    }

    pub fn patch(mut self, other: Style) -> Style {
        self.fg = other.fg.or(self.fg);
        self.bg = other.bg.or(self.bg);

        self.add_modifier.remove(other.sub_modifier);
        self.add_modifier.insert(other.add_modifier);
        self.sub_modifier.remove(other.add_modifier);
        self.sub_modifier.insert(other.sub_modifier);

        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Color {
    Reset,
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    Gray,
    DarkGray,
    LightRed,
    LightGreen,
    LightYellow,
    LightBlue,
    LightMagenta,
    LightCyan,
    White,
}

impl Color {
    pub fn from_str_or_default(name: impl AsRef<str>, default: Color) -> Self {
        match Self::from_str(name) {
            Some(color) => color,
            None => default,
        }
    }

    pub fn from_str(name: impl AsRef<str>) -> Option<Self> {
        let color = match name.as_ref() {
            "black" => Self::Black,
            "red" => Self::Red,
            "green" => Self::Green,
            "yellow" => Self::Yellow,
            "blue" => Self::Blue,
            "magenta" => Self::Magenta,
            "cyan" => Self::Cyan,
            "gray" => Self::Gray,
            "darkgray" => Self::DarkGray,
            "lightred" => Self::LightRed,
            "lightgreen" => Self::LightGreen,
            "lightyellow" => Self::LightYellow,
            "lightblue" => Self::LightBlue,
            "lightmagenta" => Self::LightMagenta,
            "lightcyan" => Self::LightCyan,
            "white" => Self::White,
            _ => return None,
        };
        Some(color)
    }

    pub fn description(self) -> &'static str {
        match self {
            Color::Reset => "reset",
            Color::Black => "black",
            Color::Red => "red",
            Color::Green => "green",
            Color::Yellow => "yellow",
            Color::Blue => "blue",
            Color::Magenta => "magenta",
            Color::Cyan => "cyan",
            Color::Gray => "gray",
            Color::DarkGray => "darkgray",
            Color::LightRed => "lightred",
            Color::LightGreen => "lightgreen",
            Color::LightYellow => "lightyellow",
            Color::LightBlue => "lightblue",
            Color::LightMagenta => "lightmagenta",
            Color::LightCyan => "lightcyan",
            Color::White => "white",
        }
    }
}

bitflags! {
    pub struct Modifier: u16 {
        const BOLD              = 0b0000_0000_0001;
        const DIM               = 0b0000_0000_0010;
        const ITALIC            = 0b0000_0000_0100;
        const UNDERLINED        = 0b0000_0000_1000;
        const SLOW_BLINK        = 0b0000_0001_0000;
        const RAPID_BLINK       = 0b0000_0010_0000;
        const REVERSED          = 0b0000_0100_0000;
        const HIDDEN            = 0b0000_1000_0000;
        const CROSSED_OUT       = 0b0001_0000_0000;
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    #[test]
    fn test_display_style() {
        let s = format!(
            "{}hello{}world",
            Style::default()
                .fg(Color::Red)
                .bg(Color::Green)
                .add_modifier(Modifier::SLOW_BLINK),
            Style::default().add_modifier(Modifier::REVERSED)
        );
        println!("{}", s);
    }
}
