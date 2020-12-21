pub mod ansi;
pub mod mxp;
pub mod cli;

use crate::ui::span::Span;
use crate::ui::style::{Style, Modifier};
use ansi::apply_sgr;
use mxp::{Tokenizer, Token, Tokenization, Mode};

/// 精简后的MXP标签，主要用于MXP触发器
#[derive(Debug, Clone, PartialEq)]
pub enum Label {
    None,
    // 超链接
    A{
        href: String,
        hint: String,
    },
    // 标题
    H(u8),
    // 发送命令
    S{
        href: String,
        hint: String,
    }
}

#[derive(Debug)]
pub struct LabelStack {
    stack: Vec<Label>,
    seq: usize,
}

impl Default for LabelStack {
    fn default() -> Self {
        Self{stack: Vec::new(), seq: 0}
    }
}

impl LabelStack {

    pub fn reset(&mut self) {
        self.stack.clear();
        self.seq = 0;
    }

    pub fn push(&mut self, label: Label) {
        if !self.stack.is_empty() {
            log::debug!("structural labels not supportted, dropping previous labels {:?}", self.stack);
            self.stack.clear();
        }
        self.stack.push(label);
    }

    pub fn peek(&self) -> Option<&Label> {
        self.stack.last()
    }

    // 输出当前Label及索引，然后索引自增
    pub fn get_and_inc(&mut self) -> (usize, Label) {
        if let Some(top) = self.stack.last() {
            let seq = self.seq;
            self.seq += 1;
            return (seq, top.clone());
        }
        (0, Label::None)
    }
}

// 缓存与合并MXP事件
// 用于将标签相同而格式不同的MXP文本合并
// 专用于MXP触发器
#[derive(Debug, Clone)]
pub struct InlineElements {
    arr: Vec<Element>,
    // MXP文本是否可连续
    cont: bool,
}

impl InlineElements {

    pub fn new() -> Self {
        Self{
            arr: Vec::new(),
            cont: false,
        }
    }

    pub fn push(&mut self, elem: Element) {
        match elem {
            Element::MxpMode(_) | Element::MxpVersion | Element::MxpSupport | Element::MxpImg(_) => {
                self.arr.push(elem);
                self.cont = false;
            }
            Element::Span(span) if span.label != Label::None => {
                if !self.cont {
                    self.arr.push(Element::Span(span));
                    self.cont = true;
                    return;
                }
                // 必然存在span
                let prev = self.arr.last_mut().unwrap().as_span_mut().unwrap();
                if span.label == prev.label {
                    // label相同时，合并文本，忽略格式的差异
                    prev.content.push_str(&span.content);
                    return;
                }
                // label不同，无法合并
                self.arr.push(Element::Span(span));
            }
            Element::Span(_) | Element::None => {
                self.cont = false;
            }
        }
    }

    pub fn to_vec(self) -> Vec<Element> {
        self.arr
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Element {
    None,
    Span(Span),
    MxpSupport,
    MxpVersion,
    MxpMode(Mode),
    MxpImg(String),
}

impl Element {

    pub fn is_span(&self) -> bool {
        match self {
            Element::Span(_) => true,
            _ => false,
        }
    }

    pub fn as_span(&self) -> Option<&Span> {
        match self {
            Element::Span(span) => Some(span),
            _ => None,
        }
    }

    pub fn as_span_mut(&mut self) -> Option<&mut Span> {
        match self {
            Element::Span(span) => Some(span),
            _ => None,
        }
    }
}

#[derive(Debug, Default)]
pub struct Parser {
    style: Style,
    tokenizer: Tokenizer,
    // label stack
    ls: LabelStack,
    buf: String,
    immediate: Option<Element>,
}

impl Parser {

    pub fn fill(&mut self, input: &str) {
        self.tokenizer.fill(input);
    }

    /// 获取下一个元素
    ///
    /// 1. 首先驱动MXP Parser对缓存的输入进行解析。
    /// 2. 然后循环获取token，并进行如下处理：
    /// 如果遇到SGR，解析并修改当前style；
    /// 如果遇到文本，存入buf；
    /// 如果遇到<A>,<H>,<S>，存入labels；
    /// 如果遇到格式token如<COLOR>,<I>,<B>等，输入已缓存文本（如有），并修改当前style；
    /// 如果遇到无法识别的文本，以默认格式输出。
    /// 直到无token返回。
    pub fn next(&mut self) -> Element {
        if let Some(im) = self.immediate.take() {
            return im;
        }
        loop {
            match self.tokenizer.next() {
                Tokenization::Pending => {
                    // buf非空时返回文本
                    if !self.buf.is_empty() {
                        return self.output(true);
                    }
                    return Element::None;
                }
                Tokenization::Invalid(s) => {
                    if self.buf.is_empty() {
                        self.buf = s;
                    } else {
                        self.buf.push_str(&s);
                    }
                    return self.output(true);
                }
                Tokenization::Ok(token) => {
                    match token {
                        // 文本
                        Token::LineEndedText(s) => {
                            if self.buf.is_empty() {
                                self.buf = s;
                            } else {
                                self.buf.push_str(&s);
                            }
                            return self.output(true);
                        }
                        Token::Text(s) => {
                            if self.buf.is_empty() {
                                self.buf = s;
                            } else {
                                self.buf.push_str(&s);
                            }
                        }
                        Token::AmperChar(c) => {
                            self.buf.push(c);
                        }
                        Token::Nbsp => {
                            self.buf.push(' ');
                        }
                        // 额外属性
                        Token::A{href, hint, ..} => {
                            let elem = self.output(false);
                            self.ls.push(Label::A{href, hint});
                            if elem.is_span() {
                                return elem;
                            }
                        }
                        Token::AEnd => {
                            if let Some(Label::A{..}) = self.ls.peek() {
                                return self.output_and_clear_label(true);
                            }
                        }
                        Token::Send{href, hint, ..} => {
                            let elem = self.output(false);
                            self.ls.push(Label::S{href, hint});
                            if elem.is_span() {
                                return elem;
                            }
                        }
                        Token::SendEnd => {
                            if let Some(Label::S{..}) = self.ls.peek() {
                                return self.output_and_clear_label(true);
                            }
                        }
                        Token::Header(n, true) => {
                            let elem = self.output(false);
                            self.ls.push(Label::H(n));
                            if elem.is_span() {
                                return elem;
                            }
                        }
                        Token::Header(_, false) => {
                            if let Some(Label::H(_)) = self.ls.peek() {
                                return self.output_and_clear_label(true);
                            }
                        }
                        // 格式类
                        Token::Bold(bold) => {
                            let new_style = if bold {
                                self.style.add_modifier(Modifier::BOLD)
                            } else {
                                self.style.remove_modifier(Modifier::BOLD)
                            };
                            if self.style == new_style {
                                continue;
                            }
                            if let Element::Span(span) = self.output_with_new_style(new_style) {
                                return Element::Span(span);
                            }
                        }
                        Token::Italic(italic) => {
                            let new_style = if italic {
                                self.style.add_modifier(Modifier::ITALIC)
                            } else {
                                self.style.remove_modifier(Modifier::ITALIC)
                            };
                            if self.style == new_style {
                                continue;
                            }
                            if let Element::Span(span) = self.output_with_new_style(new_style) {
                                return Element::Span(span);
                            }
                        }
                        Token::Underline(underline) => {
                            let new_style = if underline {
                                self.style.add_modifier(Modifier::UNDERLINED)
                            } else {
                                self.style.remove_modifier(Modifier::UNDERLINED)
                            };
                            if let Element::Span(span) = self.output_with_new_style(new_style) {
                                return Element::Span(span);
                            }
                        }
                        Token::Strikeout(strikeout) => {
                            let new_style = if strikeout {
                                self.style.add_modifier(Modifier::CROSSED_OUT)
                            } else {
                                self.style.remove_modifier(Modifier::CROSSED_OUT)
                            };
                            if let Element::Span(span) = self.output_with_new_style(new_style) {
                                return Element::Span(span);
                            }
                        }
                        Token::Color{fg, bg} => {
                            let new_style = {
                                let s = self.style.fg(fg);
                                if let Some(bg) = bg {
                                    s.bg(bg)
                                } else {
                                    s
                                }
                            };
                            if self.style == new_style {
                                continue;
                            }
                            if let Element::Span(span) = self.output_with_new_style(new_style) {
                                return Element::Span(span);
                            }
                        }
                        Token::ColorReset => {
                            let new_style = {
                                let mut s = self.style;
                                s.bg = None;
                                s.fg = None;
                                s
                            };
                            if self.style == new_style {
                                continue;
                            }
                            if let Element::Span(span) = self.output_with_new_style(new_style) {
                                return Element::Span(span);
                            }
                        }
                        Token::SGR(sgr) => {
                            let new_style = apply_sgr(self.style, &sgr);
                            if self.style == new_style {
                                continue;
                            }
                            if let Element::Span(span) = self.output_with_new_style(new_style) {
                                return Element::Span(span);
                            }
                        }
                        // 特殊指令
                        Token::Support => {
                            let elem = self.output(false);
                            if elem.is_span() {
                                self.immediate = Some(Element::MxpSupport);
                                return elem;
                            }
                            return Element::MxpSupport;
                        }
                        Token::Version => {
                            let elem = self.output(false);
                            if elem.is_span() {
                                self.immediate = Some(Element::MxpVersion);
                                return elem;
                            }
                            return Element::MxpVersion;
                        }
                        Token::MxpMode(mode) => {
                            let elem = self.output(false);
                            if elem.is_span() {
                                self.immediate = Some(Element::MxpMode(mode));
                                return elem;
                            }
                            return Element::MxpMode(mode);
                        }
                        Token::Img(url) => {
                            let elem = self.output(false);
                            if elem.is_span() {
                                self.immediate = Some(Element::MxpImg(url));
                                return elem;
                            }
                            return Element::MxpImg(url);
                        }
                        // 忽略以下token
                        Token::Expire(_) | Token::High(_) | Token::P(_) | 
                        Token::Font{..} | Token::FontReset | 
                        Token::NoBr | Token::Br | Token::Sbr | Token::None => (),
                    }
                }
            }
        }
    }

    fn output_with_new_style(&mut self, new_style: Style) -> Element {
        if self.buf.is_empty() {
            self.style = new_style;
            return Element::None;
        }
        // 暂时不使用序号
        let (_, label) = self.ls.get_and_inc();
        let span = Span::new(
            std::mem::replace(&mut self.buf, String::new()),
            self.style,
            label);
        self.style = new_style;
        Element::Span(span)
    }

    fn output(&mut self, force: bool) -> Element {
        if self.has_output() || force {
            let (_, label) = self.ls.get_and_inc();
            return Element::Span(Span::new(
                std::mem::replace(&mut self.buf, String::new()),
                self.style,
                label));
        }
        Element::None
    }

    fn output_and_clear_label(&mut self, force: bool) -> Element {
        let elem = self.output(force);
        // 这里清空所有层级的标签
        // 目前暂不考虑标签嵌套结构的情况
        // 对任意的MXP开闭标签对（A，SEND，Hn），仅最后一个开标签生效。
        self.ls.reset();
        elem
    }

    fn has_output(&self) -> bool {
        !self.buf.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::style::Color;

    #[test]
    fn test_parser_newline_text() {
        let mut parser = Parser::default();
        parser.fill("张三走了过来。\r\n");
        let elem = parser.next();
        assert_eq!(
            text("张三走了过来。\r\n"),
            elem,
        );
    }

    #[test]
    fn test_parser_partial_text() {
        let mut parser = Parser::default();
        parser.fill("张三走了");
        let elem = parser.next();
        assert_eq!(
            text("张三走了"),
            elem,
        );
        parser.fill("过来。\r\n");
        let elem = parser.next();
        assert_eq!(
            text("过来。\r\n"),
            elem,
        ); 
    }

    #[test]
    fn test_parser_non_strict_amper() {
        let mut parser = Parser::default();
        parser.fill("张三 & 李四");
        let elem = parser.next();
        assert_eq!(
            text("张三 & 李四"),
            elem,
        );
    }

    #[test]
    fn test_parser_non_strict_gt() {
        let mut parser = Parser::default();
        parser.fill("你可以看看<node>");
        let elem = parser.next();
        assert_eq!(
            text("你可以看看<node>"),
            elem,
        );

        parser.fill("你可以看看(look):baoku,shudong,<\x1b[1;31mnode\x1b[2;37;0m>");
        assert_eq!(text("你可以看看(look):baoku,shudong,<"), parser.next());
        let s1 = styled_text("node", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD));
        let s2 = parser.next();
        assert_eq!(s1, s2);
        assert_eq!(text(">"), parser.next());
    }

    #[test]
    fn test_parser_sgr_in_header() {
        let mut parser = Parser::default();
        // pkuxkx中客店的MXP序列
        parser.fill("\x1b[1z<H2>客店 - [\x1b[1;31m大宋国\x1b[2;37;0m] [\x1b[1;36m城内\x1b[2;37;0m]\x1b[2;37;0m [\x1b[1;32m存盘点\x1b[2;37;0m] </H2>\r\x00\r\n");
        let expected = vec![
            // \x1b[1z
            Element::MxpMode(Mode::Secure),
            // <H2>客店 - [
            Element::Span(Span::new("客店 - [", Style::default(), Label::H(2))),
            // \x1b[1;31m大宋国
            Element::Span(Span::new("大宋国", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD), Label::H(2))),
            // \x1b[2;37;0m] [
            Element::Span(Span::new("] [", Style::default(), Label::H(2))),
            // \x1b[1;36m城内
            Element::Span(Span::new("城内", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD), Label::H(2))),
            // \x1b[2;37;0m]
            // \x1b[2;37;0m [
            // 因为格式未发生变化，两段文本合并
            Element::Span(Span::new("] [", Style::default(), Label::H(2))),
            // \x1b[1;32m存盘点
            Element::Span(Span::new("存盘点", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD), Label::H(2))),
            // \x1b[2;37;0m] </H2>
            Element::Span(Span::new("] ", Style::default(), Label::H(2))),
            // \r\x00\r\n
            Element::Span(Span::new("\r\n", Style::default(), Label::None)),
        ];
        let mut expected = expected.into_iter();
        while let Some(elem) = expected.next() {
            assert_eq!(elem, parser.next());
        }
    }

    #[test]
    fn test_parser_inline_elements() {
        let mut parser = Parser::default();
        // pkuxkx中客店的MXP序列
        parser.fill("\x1b[1z<H2>客店 - [\x1b[1;31m大宋国\x1b[2;37;0m] [\x1b[1;36m城内\x1b[2;37;0m]\x1b[2;37;0m [\x1b[1;32m存盘点\x1b[2;37;0m] </H2>\r\x00\r\n");
        let expected = vec![
            Element::MxpMode(Mode::Secure),
            Element::Span(Span::new("客店 - [大宋国] [城内] [存盘点] ", Style::default(), Label::H(2))),
        ];
        let mut inliner = InlineElements::new();
        loop {
            match parser.next() {
                Element::None => break,
                elem => inliner.push(elem),
            }
        }
        let actual = inliner.to_vec();
        assert_eq!(expected, actual);
    }
    
    fn text(text: impl Into<String>) -> Element {
        Element::Span(Span::new(text, Style::default(), Label::None))
    }

    fn styled_text(text: impl Into<String>, style: Style) -> Element {
        Element::Span(Span::new(text, style, Label::None))
    }

}