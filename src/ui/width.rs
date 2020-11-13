use crate::ui::span::ArcSpan;
use crate::ui::line::Line;
use unicode_width::UnicodeWidthChar;

pub trait DisplayWidthTab8 {
    const TAB_SPACES: usize = 8;
    
    fn display_width(&self, cjk: bool) -> usize;

}

impl DisplayWidthTab8 for str {

    fn display_width(&self, cjk: bool) -> usize {
        self.chars().fold(0, |s, c| {
            if c == '\t' {
                s / Self::TAB_SPACES * Self::TAB_SPACES + Self::TAB_SPACES
            } else {
                s + if cjk { c.width_cjk().unwrap_or(0) } else { c.width_cjk().unwrap_or(0) }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::DisplayWidthTab8;
    use std::io::Read;
    use std::fs::File;

    #[test]
    fn test_str_width() {

        let s = "1234567812345678123456781234567812345678\t";
        println!("{}|", s);
        println!("width={}", s.display_width(true));


        // let set = HashSet::new();

        // let mut f = File::open("server.log").unwrap();
        // let mut s = String::new();
        // f.read_to_string(&mut s).unwrap();

        // for c in s.chars() {
        //     if c.width_cjk().is_none() && c != '\x1b' && c != '\r' && c != '\n' {
        //         println!("char={}, hex={:x}", c, c as u32);
        //     }
        // }
    }
}