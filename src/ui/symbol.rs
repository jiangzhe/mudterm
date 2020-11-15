pub const VERTICAL: char = '│';
pub const HORIZONTAL: char = '─';
pub const TOP_RIGHT: char = '┐';
pub const ROUNDED_TOP_RIGHT: char = '╮';
pub const TOP_LEFT: char = '┌';
pub const ROUNDED_TOP_LEFT: char = '╭';
pub const BOTTOM_RIGHT: char = '┘';
pub const ROUNDED_BOTTOM_RIGHT: char = '╯';
pub const BOTTOM_LEFT: char = '└';
pub const ROUNDED_BOTTOM_LEFT: char = '╰';

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthChar;

    #[test]
    fn test_symbol_width() {
        for s in vec![
            VERTICAL,
            HORIZONTAL,
            TOP_RIGHT,
            ROUNDED_TOP_RIGHT,
            TOP_LEFT,
            ROUNDED_TOP_LEFT,
            BOTTOM_RIGHT,
            ROUNDED_BOTTOM_RIGHT,
            BOTTOM_LEFT,
            ROUNDED_BOTTOM_LEFT,
        ] {
            println!("{}.width={:?},cjk_width={:?}", s, s.width(), s.width_cjk());
        }
    }

    #[test]
    fn test_draw_border() {
        let mut line1 = String::new();
        line1.push(ROUNDED_TOP_LEFT);
        for _ in 0..5 {
            line1.push(HORIZONTAL);
        }
        line1.push(ROUNDED_TOP_RIGHT);
        println!("{}", line1);

        let mut line2 = String::new();
        line2.push(VERTICAL);
        for _ in 0..5 {
            line2.push_str(" ");
        }
        line2.push(VERTICAL);
        println!("{}", line2);

        let mut line3 = String::new();
        line3.push(ROUNDED_BOTTOM_LEFT);
        for _ in 0..5 {
            line3.push(HORIZONTAL);
        }
        line3.push(ROUNDED_BOTTOM_RIGHT);
        println!("{}", line3);
    }
}
