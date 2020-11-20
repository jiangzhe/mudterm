use crate::ui::ansi::AnsiParser;
use crate::ui::style::Style;
use crate::ui::span::Span;
use crate::error::Result;
use std::collections::VecDeque;
use std::collections::vec_deque::Iter;

#[derive(Debug)]
pub struct MultilineColorString {
    // 存放纯文本
    raw: String,
    // 最小行数，也是文本匹配的最大行数
    min_lines: usize,
    // 最大行数，超过则自动缩容
    max_lines: usize,
    // 首字符偏移量，当存放行数超过最小行数时不为0
    offset: usize,
    // 每行的字节偏移与格式信息
    idx: VecDeque<(usize, Vec<InlineStyle>)>,
    // ANSI解析器
    parser: AnsiParser,
}

impl MultilineColorString {

    pub fn new(min_lines: usize, max_lines: usize) -> Self {
        let mcs = MultilineColorString{
            raw: String::new(),
            min_lines,
            max_lines,
            offset: 0,
            idx: VecDeque::new(),
            parser: AnsiParser::new(),
        };
        // todo: fill with min lines
        mcs
    }

    pub fn lastn(&self, n: usize) -> Result<&str> {
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct InlineStyle {
    pub start: usize,
    pub style: Style,
}


#[cfg(test)]
mod tests {
    use regex::Regex;

    #[test]
    fn test_match_carriage() {
        let re = Regex::new("(?m)^abc$").unwrap();
        println!("{}", re.is_match("abc\n"));
        println!("{}", re.is_match("abc\r\n"));
    }
}