use crate::error::{Error, Result};
use crate::ui::layout::Rect;
use crate::ui::style::{Color, Modifier, Style};
use crate::ui::width::AppendWidthTab8;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Symbol {
    pub ch: char,
    pub width: u16,
    pub exists: bool,
}

impl Symbol {
    // 占据屏幕上一个列宽的位置，但实际不存在
    pub fn empty() -> Self {
        Self {
            ch: ' ',
            width: 1,
            exists: false,
        }
    }

    pub fn new(ch: char, width: u16, exists: bool) -> Self {
        Self { ch, width, exists }
    }
}

/// 定义终端的单个字宽的位置
///
/// 每个位置可包含不超过一个字符。
/// 字符的宽度可以超过1，若为多字宽(n-width)，如中文，则其后的
/// 的n-1个位置中不应包含字符
/// 默认填充空格
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    // 字符，字符在当前位置的宽度，是否实际存在
    pub symbol: Symbol,
    pub fg: Color,
    pub bg: Color,
    pub modifier: Modifier,
}

impl Cell {
    pub fn set_symbol(&mut self, symbol: Symbol) -> &mut Cell {
        self.symbol = symbol;
        self
    }

    pub fn reset_symbol(&mut self) {
        self.symbol = Symbol::empty();
    }

    /// 对制表符进行特殊处理，因为制表符宽度依赖于当前光标位置
    pub fn set_tab(&mut self, width: u16) -> &mut Cell {
        self.symbol = Symbol::new('\t', width, true);
        self
    }

    pub fn set_style(&mut self, style: Style) -> &mut Cell {
        if let Some(c) = style.fg {
            self.fg = c;
        }
        if let Some(c) = style.bg {
            self.bg = c;
        }
        self.modifier.insert(style.add_modifier);
        self.modifier.remove(style.sub_modifier);
        self
    }

    pub fn style(&self) -> Style {
        Style::default()
            .fg(self.fg)
            .bg(self.bg)
            .add_modifier(self.modifier)
    }

    pub fn reset(&mut self) {
        self.symbol = Symbol::empty();
        self.fg = Color::Reset;
        self.bg = Color::Reset;
        self.modifier = Modifier::empty();
    }
}

impl Default for Cell {
    fn default() -> Cell {
        Cell {
            symbol: Symbol::empty(),
            fg: Color::Reset,
            bg: Color::Reset,
            modifier: Modifier::empty(),
        }
    }
}

pub trait Buffer {
    /// 获取边界
    fn area(&self) -> &Rect;

    /// 获取指定行列单元，可更新
    fn get_mut(&mut self, x: u16, y: u16) -> &mut Cell;

    /// 获取指定行列单元
    fn get(&self, x: u16, y: u16) -> &Cell;

    /// 在指定点指定指定宽度设置字符串
    ///
    /// 该字符串必须为单行，行内的\r\n都将被忽略。
    /// 超出最大宽度将自动截断。
    /// 行尾的\r\n将自动填充空格字符(' ')至指定宽度
    /// 返回为空时表示该行已占满，否则，表示当前行的光标的水平位置。
    fn set_line_str(
        &mut self,
        x: u16,
        y: u16,
        s: impl AsRef<str>,
        right: u16,
        style: Style,
        cjk: bool,
    ) -> Option<u16> {
        debug_assert!(
            x >= self.area().left() && right <= self.area().right(),
            "x={},right={} not in buffer {}..{}, s={}",
            x,
            right,
            self.area().left(),
            self.area().right(),
            s.as_ref(),
        );
        debug_assert!(
            y >= self.area().top() && y < self.area().bottom(),
            "y={} not in buffer {}..{}, s={}",
            y,
            self.area().top(),
            self.area().bottom(),
            s.as_ref(),
        );
        let s = s.as_ref();
        // 处理行结束符
        let (s, newline) = if s.ends_with("\r\n") {
            (&s[..s.len() - 2], true)
        } else if s.ends_with('\n') {
            (&s[..s.len() - 1], true)
        } else {
            (s, false)
        };

        let mut curr_x = x;
        for c in s.chars() {
            let next_x = c.append_width(curr_x as usize, cjk) as u16;
            // 目前暂时忽略特殊字符
            if next_x == curr_x {
                continue;
            }
            if next_x > right {
                // 越界，该字符无法填充到当前行，将剩余字符全部丢弃
                return None;
            } else {
                let cw = next_x - curr_x;
                // 可填充，对宽字符填充其占据的多个位置
                self.get_mut(curr_x, y)
                    .set_style(style)
                    .set_symbol(Symbol::new(c, cw, true));
                for x in curr_x + 1..next_x {
                    self.get_mut(x, y)
                        .set_style(style)
                        .set_symbol(Symbol::empty());
                }
            }
            curr_x = next_x;
        }
        // 存在行结束符时，需要将该行填满
        if newline {
            for x in curr_x..right {
                self.get_mut(x, y)
                    .set_style(style)
                    .set_symbol(Symbol::empty());
            }
            return None;
        }
        // 行已经恰好被填满
        if curr_x == right {
            return None;
        }
        Some(curr_x)
    }

    /// 比较两份缓存，并返回需要更新的单元列表
    ///
    /// 对于宽字符集，部分终端渲染可能导致字符元素的残留
    /// 这里交由Terminal进行处理，处理逻辑为先擦除再写入
    fn diff<'a, B>(&self, other: &'a B, updates: &mut Vec<(u16, u16, Cell)>)
    where
        B: Buffer,
    {
        debug_assert_eq!(
            self.area(),
            other.area(),
            "compare buffers with different areas: {:?} vs {:?}",
            self.area(),
            other.area()
        );
        let mut invalidated: u16 = 0;
        let mut to_skip: u16 = 0;

        for y in self.area().top()..self.area().bottom() {
            for x in self.area().left()..self.area().right() {
                let cc = self.get(x, y);
                let nc = other.get(x, y);
                if (cc != nc || invalidated > 0) && to_skip == 0 {
                    updates.push((x, y, nc.clone()));
                }
                to_skip = nc.symbol.width.saturating_sub(1);
                let affacted_width = std::cmp::max(nc.symbol.width, cc.symbol.width);
                invalidated = std::cmp::max(affacted_width, invalidated).saturating_sub(1);
            }
        }
    }

    fn set_style(&mut self, area: Rect, style: Style) {
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                self.get_mut(x, y).set_style(style);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BufferVec {
    area: Rect,
    content: Vec<Cell>,
}

impl BufferVec {
    pub fn empty(area: Rect) -> Self {
        let cell = Cell::default();
        Self::filled(area, &cell)
    }

    pub fn filled(area: Rect, cell: &Cell) -> Self {
        let size = area.area() as usize;
        let mut content = Vec::with_capacity(size);
        for _ in 0..size {
            content.push(cell.clone());
        }
        Self { area, content }
    }

    pub fn reset(&mut self) {
        for c in &mut self.content {
            c.reset();
        }
    }

    pub fn index_of(&self, x: u16, y: u16) -> usize {
        debug_assert!(
            x >= self.area.left()
                && x < self.area.right()
                && y >= self.area.top()
                && y < self.area.bottom(),
            "Trying to access position outside the buffer: x={}, y={}, area={:?}",
            x,
            y,
            self.area
        );
        ((y - self.area.y) * self.area.width + (x - self.area.x)) as usize
    }

    pub fn xy_of(&self, idx: usize) -> (u16, u16) {
        let y = self.area.y + (idx / self.area.width as usize) as u16;
        let x = self.area.x + (idx % self.area.width as usize) as u16;
        (x, y)
    }

    pub fn get(&self, x: u16, y: u16) -> &Cell {
        let i = self.index_of(x, y);
        &self.content[i]
    }

    pub fn get_mut(&mut self, x: u16, y: u16) -> &mut Cell {
        let i = self.index_of(x, y);
        &mut self.content[i]
    }

    pub fn subset(&mut self, area: Rect) -> Result<BufferSubset> {
        if area.left() < self.area.left()
            || area.right() > self.area.right()
            || area.top() < self.area.top()
            || area.bottom() > self.area.bottom()
        {
            return Err(Error::RuntimeError(format!(
                "illegal subset {:?} of area {:?}",
                area, self.area
            )));
        }
        Ok(BufferSubset { buffer: self, area })
    }
}

impl Buffer for BufferVec {
    fn area(&self) -> &Rect {
        &self.area
    }

    fn get(&self, x: u16, y: u16) -> &Cell {
        let i = self.index_of(x, y);
        &self.content[i]
    }

    fn get_mut(&mut self, x: u16, y: u16) -> &mut Cell {
        let i = self.index_of(x, y);
        &mut self.content[i]
    }
}

pub struct BufferSubset<'b> {
    buffer: &'b mut BufferVec,
    area: Rect,
}

impl Buffer for BufferSubset<'_> {
    fn area(&self) -> &Rect {
        &self.area
    }

    fn get(&self, x: u16, y: u16) -> &Cell {
        self.buffer.get(x, y)
    }

    fn get_mut(&mut self, x: u16, y: u16) -> &mut Cell {
        self.buffer.get_mut(x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::symbol::HORIZONTAL;
    use unicode_segmentation::UnicodeSegmentation;
    #[test]
    fn test_buffer_set_single_line() {
        let mut buffer = BufferVec::empty(Rect::new(1, 1, 5, 1));
        let pos = buffer.set_line_str(1, 1, "he", 6, Style::default(), true);
        assert_eq!(Some(3), pos);
        assert_eq!("he", single_line(&buffer));

        buffer.reset();
        assert_eq!("", single_line(&buffer));

        buffer.reset();
        let pos = buffer.set_line_str(1, 1, "hello, world", 6, Style::default(), true);
        assert_eq!(None, pos);
        assert_eq!("hello", single_line(&buffer));

        buffer.reset();
        let pos = buffer.set_line_str(1, 1, "hello", 6, Style::default(), true);
        assert!(pos.is_none());
        assert_eq!("hello", single_line(&buffer));

        buffer.reset();
        let pos = buffer.set_line_str(1, 1, "你好中国", 6, Style::default(), true);
        assert!(pos.is_none());
        assert_eq!("你好", single_line(&buffer));

        buffer.reset();
        let pos = buffer.set_line_str(1, 1, "hello", 5, Style::default(), true);
        assert!(pos.is_none());
        assert_eq!("hell", single_line(&buffer));

        buffer.reset();
        let pos = buffer.set_line_str(1, 1, "hell\r\n", 6, Style::default(), true);
        assert!(pos.is_none());
        assert_eq!("hell", single_line(&buffer));

        buffer.reset();
        let pos = buffer.set_line_str(1, 1, "hell\r\no, world", 6, Style::default(), true);
        assert!(pos.is_none());
        assert_eq!("hello", single_line(&buffer));
    }

    #[test]
    fn test_buffer_set_multi_line() {
        let mut buffer = BufferVec::empty(Rect::new(1, 1, 6, 2));
        let input = "hell\no, world";
        for (i, line) in input.split('\n').enumerate() {
            buffer.set_line_str(
                1,
                i as u16 + 1,
                line,
                1 + buffer.area.width,
                Style::default(),
                true,
            );
        }
        assert_eq!(
            multi_lines(&buffer),
            vec!["hell".to_owned(), "o, wo".to_owned(),]
        );
    }

    #[test]
    fn test_buffer_diff() {
        let mut buf1 = BufferVec::empty(Rect::new(1, 1, 5, 1));
        buf1.set_line_str(1, 1, "hell", 6, Style::default(), true);

        let mut buf2 = BufferVec::empty(Rect::new(1, 1, 5, 1));
        buf2.set_line_str(1, 1, "h中", 6, Style::default(), true);

        let mut updates = vec![];
        buf1.diff(&buf2, &mut updates);
        println!("updates={:#?}", updates);
    }

    #[test]
    fn test_buffer_move() {
        let area = Rect::new(1, 1, 10, 4);
        let mut buf1 = BufferVec::empty(area);
        buf1.set_line_str(1, 1, "静言\r\n", area.right(), Style::default(), true);
        buf1.set_line_str(
            1,
            2,
            "┌───基本知识\r\n",
            area.right(),
            Style::default(),
            true,
        );
        buf1.set_line_str(1, 3, "│ 读书\r\n", area.right(), Style::default(), true);
        buf1.set_line_str(1, 4, "│ 叫化\r\n", area.right(), Style::default(), true);
        let mut buf2 = BufferVec::empty(area);
        buf2.set_line_str(
            1,
            1,
            "┌───基本知识\r\n",
            area.right(),
            Style::default(),
            true,
        );
        buf2.set_line_str(1, 2, "│ 读书\r\n", area.right(), Style::default(), true);
        buf2.set_line_str(1, 3, "│ 叫化\r\n", area.right(), Style::default(), true);
        buf2.set_line_str(1, 4, "│ 道听\r\n", area.right(), Style::default(), true);
        let mut updates = vec![];
        buf1.diff(&buf2, &mut updates);
        println!("updates={:#?}", updates);
    }

    #[test]
    fn test_unicode_segmentation() {
        let mut s = String::new();
        s.push(HORIZONTAL);
        s.push(HORIZONTAL);
        for g in s.graphemes(true) {
            println!("{}", g);
        }
    }

    fn single_line(buffer: &BufferVec) -> String {
        buffer
            .content
            .iter()
            .filter(|c| c.symbol.exists)
            .map(|c| c.symbol.ch)
            .collect()
    }

    fn multi_lines(buffer: &BufferVec) -> Vec<String> {
        let mut ss = vec![];
        for y in buffer.area.top()..buffer.area.bottom() {
            let mut s = String::new();
            for x in buffer.area.left()..buffer.area.right() {
                let c = buffer.get(x, y);
                if c.symbol.exists {
                    s.push(c.symbol.ch);
                }
            }
            ss.push(s);
        }
        ss
    }
}
