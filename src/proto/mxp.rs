//! MUD eXtension Protocol
//!
//! A good introduction to how to implement it in MUSHClient:
//! http://www.gammon.com.au/forum/bbshowpost.php?bbsubject_id=222
use crate::ui::style::Color;

pub fn supports() -> &'static str {
    "+head +body +afk +title +username +pass +samp +h +high +i +option +bold +xch_page +reset +strong +recommend_option +support +ul +em +send +send.href +send.hint +send.xch_cmd +send.xch_hint +send.prompt +p +hr +html +user +password +a +a.href +a.xch_cmd +a.xch_hint +underline +b +img +img.src +img.xch_mode +pre +li +ol +c +c.fore +c.back +font +font.color +font.back +font.fgcolor +font.bgcolor +u +mxp +mxp.off +version +br +v +var +italic"
}

/// 目前支持MXP两种模式Open和Secure
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Open,
    Secure,
    // Locked,
    // Reset,
    // TempSecur,
    // LockOpen,
    // LockSecure,
    // LockLocked,
}

/// 定义MXP Tags
/// https://www.zuggsoft.com/zmud/mxp.htm
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // <B>, <BOLD>, <STRONG>
    Bold(bool),
    // <I>, <ITALIC>, <EM>
    Italic(bool),
    // <U>, <UNDERLINE>
    Underline(bool),
    // <S>, <STRIKEOUT>
    Strikeout(bool),
    // <C>, <COLOR FORE=... BACK=...>
    // 前景色，背景色
    Color{
        fg: Color, 
        bg: Option<Color>,
    },
    // </C>
    ColorReset,
    // <H> <HIGH>
    High(bool),
    // <FONT FACE=... SIZE=... COLOR=... BACK=...>
    // 字体名，字体大小，前景色，背景色
    Font{
        face: String, 
        size: Option<u32>, 
        fg: Option<Color>, 
        bg: Option<Color>,
    },
    // </FONT>
    FontReset,
    // <NOBR>
    // 忽略其后的\n
    NoBr,
    // <P>
    // 段落，其中所有\n被忽略
    P(bool),
    // <BR>
    // 换行，MXP模式中不自动切换模式
    Br,
    // <SBR>
    // 软换行，客户端可以使用空格替代，单在换行模式下建议换行
    Sbr,
    // &nbsp;
    // 代替空格
    Nbsp,
    // <A href=... hint=... expire=...>
    A{
        href: String,
        hint: String,
        expire: Option<String>,
    },
    AEnd,
    // <SEND href=... hint=... prompt expire=...>
    Send{
        href: String,
        hint: String,
        prompt: bool,
        expire: Option<String>,
    },
    SendEnd,
    // <EXPIRE ...>
    Expire(String),
    // <VERSION>
    // 向客户端查询MXP版本
    Version,
    // <SUPPORT>
    // 向客户端查询支持的标签列表
    Support,
    // CSI选择图形再现
    // 用于设置文本样式与颜色
    SGR(String),
    // MXP模式转换
    MxpMode(Mode),
    // amper转移字符
    AmperChar(char),
    // 文本
    Text(String),
    // 由\n结尾的文本
    LineEndedText(String),
    // H1 ~ H6
    Header(u8, bool),
    Img(String),
    None,
}

impl Token {
    pub fn default_from_str(s: &str, start: bool) -> Option<Token> {
        match &s.to_uppercase()[..] {
            "B" | "BOLD" | "STRONG" => Some(Token::Bold(start)),
            "I" | "ITALIC" | "EM" => Some(Token::Italic(start)),
            "U" | "UNDERLINE" => Some(Token::Underline(start)),
            "S" | "STRIKEOUT" => Some(Token::Strikeout(start)),
            "C" | "COLOR" => if start {
                Some(Token::Color{fg: Color::Gray, bg: None})
            }  else {
                Some(Token::ColorReset)
            }
            "H" | "HIGH" => Some(Token::High(start)),
            "FONT" => if start {
                Some(Self::new_font())
            } else {
                Some(Token::FontReset)
            }
            "NOBR" if start => Some(Token::NoBr),
            "P" => Some(Token::P(start)),
            "BR" if start => Some(Token::Br),
            "SBR" if start => Some(Token::Sbr),
            "A" => if start {
                Some(Self::new_a())
            } else {
                Some(Token::AEnd)
            }
            "SEND" => if start {
                Some(Self::new_send())
            } else {
                Some(Token::SendEnd)
            }
            "EXPIRE" if start => Some(Token::Expire(String::new())),
            "VERSION" if start => Some(Token::Version),
            "SUPPORT" if start => Some(Token::Support),
            "H1" => Some(Token::Header(1, start)),
            "H2" => Some(Token::Header(2, start)),
            "H3" => Some(Token::Header(3, start)),
            "H4" => Some(Token::Header(4, start)),
            "H5" => Some(Token::Header(5, start)),
            "H6" => Some(Token::Header(6, start)),
            "IMG" => Some(Token::Img(String::new())),
            _ => None,
        }
    }

    pub fn new_font() -> Self {
        Token::Font{face: String::new(), size: None, fg: None, bg: None}
    }

    pub fn new_a() -> Self {
        Token::A{href: String::new(), hint: String::new(), expire: None}
    }

    pub fn new_send() -> Self {
        Token::Send{href: String::new(), hint: String::new(), prompt: false, expire: None}
    }

    pub fn is_a(&self) -> bool {
        match self {
            Token::A{..} => true,
            _ => false,
        }
    }

    pub fn is_send(&self) -> bool {
        match self {
            Token::Send{..} => true,
            _ => false,
        }
    }

    pub fn apply_attr_value(&mut self, attr_name: &str, attr_value: &str) {
        match self {
            Token::Color{fg, bg} => {
                match attr_name {
                    "FORE" => if let Some(cl) = Color::from_str(attr_value) {
                        *fg = cl;
                    }
                    "BACK" => if let Some(cl) = Color::from_str(attr_value) {
                        *bg = Some(cl);
                    }
                    _ => (),
                }
            }
            Token::Font{face, size, fg, bg} => {
                match attr_name {
                    "FACE" => *face = attr_value.to_owned(),
                    "SIZE" => if let Ok(sz) = attr_value.parse() {
                        *size = Some(sz);
                    }
                    "COLOR" => if let Some(cl) = Color::from_str(attr_value) {
                        *fg = Some(cl);
                    }
                    "BACK" => if let Some(cl) = Color::from_str(attr_value) {
                        *bg = Some(cl);
                    }
                    _ => (),
                }
            }
            Token::A{href, hint, expire} => {
                match attr_name {
                    "HREF" => *href = attr_value.to_owned(),
                    "HINT" => *hint = attr_value.to_owned(),
                    "EXPIRE" => *expire = Some(attr_value.to_owned()),
                    _ => (),
                }
            }
            Token::Send{href, hint, prompt, expire} => {
                match attr_name {
                    "HREF" => *href = attr_value.to_owned(),
                    "HINT" => *hint = attr_value.to_owned(),
                    "PROMPT" => if let Ok(p) = attr_value.parse() {
                        *prompt = p;
                    }
                    "EXPIRE" => *expire = Some(attr_value.to_owned()),
                    _ => (),
                }
            }
            Token::Img(s) => {
                match attr_name {
                    "SRC" => *s = attr_value.to_owned(),
                    _ => (),
                }
            }
            _ => (),
        }
    }

    pub fn apply_attr(&mut self, attr_name: &str) {
        match self {
            Token::Send{prompt, ..} if attr_name == "PROMPT" => *prompt = true,
            Token::Expire(s) => *s = attr_name.to_owned(),
            _ => (),
        }
    }

    // 对于某些标签，应用第一个无值标签名，等同于将传入值设置为其主属性值
    // 例如： <FONT "Times New Roman">  ==  <FONT FACE="Times New Roman">
    //       <C red>  ==  <C FORE=red>
    pub fn apply_first_attr(&mut self, attr_name: &str) {
        match self {
            Token::Expire(s) | Token::Img(s) => *s = attr_name.to_owned(),
            Token::A{href, ..} => *href = attr_name.to_owned(),
            Token::Font{face, ..} => *face = attr_name.to_owned(),
            Token::Color{fg, ..} => if let Some(cl) = Color::from_str(attr_name) {
                *fg = cl;
            }
            _ => (),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Tokenization {
    Ok(Token),
    // 仅存在于严格模式下
    Invalid(String),
    // 等待新的输入
    Pending,
}

impl Tokenization {

    pub fn invalid(&self) -> bool {
        match self {
            Tokenization::Invalid(_) => true,
            _ => false,
        }
    }

    pub fn pending(&self) -> bool {
        match self {
            Tokenization::Pending => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParserState {
    // 状态转移： Normal => StartTagOpen|Esc|Amper|Normal|Normal(newline)
    Normal(usize),
    // <
    // 状态转移： StartTagOpen => StartTagName|EndTagOpen|Normal(invalid char)
    StartTagOpen(usize),
    // <A
    // 状态转移： StartTagName => StartTagName|TagWhitespace|Normal(close)|Normal(invalid char)
    StartTagName{
        start: usize,
        end: usize,
    },
    // </
    // 状态转移： EndTagOpen => EndTagName|Normal(invalid char)
    EndTagOpen(usize),
    // </A
    // 状态转移： EndTagName => EndTagName|TagEndClose|Normal(invalid char)
    EndTagName{
        start: usize,
        end: usize,
    },
    // <A\s
    // 状态转移： TagWhitespace => TagWhitespace|TagAttrName|TagAttrQuoteNameOpen|Normal(close)|Normal(invalid char)
    TagWhitespace(usize),
    // <A href
    // 状态转移： TagAttrName => TagAttrAssign|TagWhitespace|TagAttrName|Normal(close)|Normal(invalid char)
    TagAttrName{
        start: usize,
        end: usize,
    },
    // <FONT "
    // 状态转移： TagAttrQuoteNameOpen => TagAttrQuoteName|Normal(invalid char)
    TagAttrQuoteNameOpen(usize),
    // <FONT "sim
    // 状态转移： TagAttrQuoteName => TagAttrQuoteName|TagAttrQuoteNameClose
    TagAttrQuoteName{
        start: usize,
        end: usize,
    },
    // <FONT "simsun"
    // 状态转移： TagAttrQuoteNameClose => TagAttrAssign|TagWhitespace|Normal(close)|Normal(invalid char)
    TagAttrQuoteNameClose(usize),
    // <A href=
    // 状态转移： TagAttrAssign => TagAttrWhitespace|Normal(close)|TagAttrQuoteValueOpen|TagAttrValue
    TagAttrAssign(usize),
    // <A href=abc
    // 状态转移： TagAttrValue => TagAttrWhitespace|Normal(close)|TagAttrValue
    TagAttrValue{
        start: usize,
        end: usize,
    },
    // <A href="
    // 状态转移： TagAttrQuoteValueOpen => TagAttrQuoteValue|TagAttrQuoteValueClose
    TagAttrQuoteValueOpen(usize),
    // <A href="abc
    // 状态转移： TagAttrQuoteValue => TagAttrQuoteValueClose|TagAttrQuoteValue
    TagAttrQuoteValue{
        start: usize,
        end: usize,
    },
    // <A href="abc"
    // 状态转移： TagAttrQuoteValueClose => TagWhitespace|Normal(close)|Normal(invalid char)
    TagAttrQuoteValueClose(usize),
    // ESC
    // 状态转移： Esc => EscBracket|Normal(invalid char)
    Esc(usize),
    // ESC[
    // 状态转移： EscBracket => CSI|Normal(CSI reset)|Normal(invalid char)
    EscBracket(usize),
    // ESC[m;n
    // 状态转移： CSI => CSI|Normal(CSI complete)|Normal(MXP Mode)|Normal(invalid char)
    CSI{
        start: usize,
        end: usize,
    },
    // &
    // 状态转移： Amper => Normal(semicomma terminate)|Normal(length limit)|Normal(invalid char)
    Amper{
        start: usize,
        end: usize,
    },
}

impl ParserState {

    pub fn start(&self) -> usize {
        match self {
            ParserState::Normal(n) | 
            ParserState::StartTagOpen(n) | 
            ParserState::EndTagOpen(n) |
            ParserState::TagWhitespace(n) | 
            ParserState::TagAttrAssign(n) | 
            ParserState::TagAttrQuoteNameOpen(n) |
            ParserState::TagAttrQuoteNameClose(n) |
            ParserState::TagAttrQuoteValueOpen(n) | 
            ParserState::TagAttrQuoteValueClose(n) |
            ParserState::Esc(n) |
            ParserState::EscBracket(n) => *n,
            ParserState::StartTagName{end, ..} | 
            ParserState::EndTagName{end, ..} |
            ParserState::TagAttrName{end, ..} | 
            ParserState::TagAttrValue{end, ..} |
            ParserState::TagAttrQuoteName{end, ..} |
            ParserState::TagAttrQuoteValue{end, ..} |
            ParserState::CSI {end, ..} |
            ParserState::Amper{end, ..} => *end,
        }
    }

    pub fn is_normal(&self) -> bool {
        match self {
            ParserState::Normal(_) => true,
            _ => false,
        }
    }
}

#[derive(Debug)]
pub struct Tokenizer {
    mode: Mode,
    state: ParserState,
    buf: String,
    token: Option<Token>,
    attr_name: Option<String>,
    n_applies: usize,
    // 由于MUD服务可能不严格遵守MXP协议，
    // 如pkuxkx，在普通文本中并未对'<', '>', '&'
    // 进行转义处理。可能导致大量信息被判定
    // 为不合法，这里可默认宽松处理，即如果解析结果
    // 不合法，直接认定为普通文本进行处理，
    // 所以状态机增加从各中间状态返回正常状态的变换
    strict: bool,
}

impl Default for Tokenizer {
    fn default() -> Self {
        Self{
            mode: Mode::Open,
            state: ParserState::Normal(0),
            buf: String::new(),
            token: None,
            attr_name: None,
            n_applies: 0,
            strict: false,
        }
    }
}

impl Tokenizer {

    // 创建一个严格模式的Tokenizer
    pub fn strict() -> Self {
        Self{
            strict: true,
            ..Default::default()
        }
    }

    // 填充字符串
    pub fn fill(&mut self, input: &str) {
        self.buf.push_str(input);
    }

    // 解析缓存中的token
    pub fn next(&mut self) -> Tokenization {
        let idx = self.state.start();
        if self.state.is_normal() && idx == self.buf.len() {
            return Tokenization::Pending;
        }
        let Self{mode, state, buf, 
            token, attr_name, n_applies, strict} = self;
        for c in buf[idx..].chars() {
            match state {
                ParserState::Normal(offset) => {
                    match c {
                        // 只在严格模式或者MXP安全模式下，才进行标签解析
                        '<' if *strict || *mode == Mode::Secure => {
                            if *offset > idx {
                                let text = Self::unify_text(buf, idx, *offset);
                                self.state = ParserState::StartTagOpen(*offset+1);
                                return Tokenization::Ok(Token::Text(text));
                            }
                            *state = ParserState::StartTagOpen(*offset+1);
                        }
                        '\x1b' => {
                            if *offset > idx {
                                let text = Self::unify_text(buf, idx, *offset);
                                self.state = ParserState::Esc(*offset+1);
                                return Tokenization::Ok(Token::Text(text));
                            }
                            *state = ParserState::Esc(*offset+1);
                        }
                        // 只在严格模式或者MXP安全模式下，才进行html转义解析
                        '&' if *strict || *mode == Mode::Secure => {
                            if *offset > idx {
                                let text = Self::unify_text(buf, idx, *offset);
                                *state = ParserState::Amper{
                                    start: *offset,
                                    end: *offset+1,
                                };
                                return Tokenization::Ok(Token::Text(text));
                            }
                            *state = ParserState::Amper{
                                start: *offset,
                                end: *offset+1,
                            };
                        }
                        '\n' => {
                            let mut text = Self::unify_text(buf, idx, *offset);
                            // 行尾无'\r'则补齐'\r'
                            if !text.ends_with('\r') {
                                text.push('\r');
                            }
                            text.push('\n');
                            // 根据MXP协议，换行切换为Open模式
                            // *mode = Mode::Open;
                            // *state = ParserState::Normal(*offset+1);
                            // buf.clear();
                            // *state = ParserState::Normal(0);
                            self.reset();
                            return Tokenization::Ok(Token::LineEndedText(text));
                        }
                        _ => {
                            *offset += c.len_utf8();
                        }
                    }
                }
                ParserState::StartTagOpen(offset) => {
                    match c {
                        '/' => {
                            *state = ParserState::EndTagOpen(*offset+1);
                        }
                        'a'..='z' | 'A'..='Z' => {
                            *state = ParserState::StartTagName{
                                start: *offset,
                                end: *offset+1,
                            };
                        }
                        _ if *strict => return self.invalidate(idx),
                        _ => *state = ParserState::Normal(*offset+c.len_utf8()),
                    }
                }
                ParserState::EndTagOpen(offset) => {
                    match c {
                        'a'..='z' | 'A'..='Z' => {
                            *state = ParserState::EndTagName{
                                start: *offset,
                                end: *offset+1,
                            };
                        }
                        _ if *strict => return self.invalidate(idx),
                        _ => *state = ParserState::Normal(*offset+c.len_utf8()),
                    }
                }
                ParserState::StartTagName{start, end} => {
                    match c {
                        'a'..='z' | 'A'..='Z' | '0'..='9' => {
                            *end += 1;
                        }
                        ' ' | '\t' => {
                            let tag_name = &buf[*start..*end];
                            match Token::default_from_str(tag_name, true) {
                                Some(tk) => {
                                    *token = Some(tk);
                                    *state = ParserState::TagWhitespace(*end + 1);
                                }
                                None if *strict => return self.invalidate(idx),
                                _ => *state = ParserState::Normal(*end+1),
                            }
                        }
                        '>' => {
                            let tag_name = &buf[*start..*end];
                            match Token::default_from_str(tag_name, true) {
                                Some(tk) => {
                                    self.state = ParserState::Normal(*end+1);
                                    return Tokenization::Ok(tk);
                                }
                                None if *strict => return self.invalidate(idx),
                                _ => *state = ParserState::Normal(*end+1),
                            }
                        }
                        _ if *strict => return self.invalidate(idx),
                        _ => *state = ParserState::Normal(*end+c.len_utf8()),
                    }
                }
                ParserState::EndTagName{start, end} => {
                    match c {
                        'a'..='z' | 'A'..='Z' | '0'..='9' => {
                            *end += 1;
                        }
                        '>' => {
                            let tag_name = &buf[*start..*end];
                            match Token::default_from_str(tag_name, false) {
                                Some(tk) => {
                                    *state = ParserState::Normal(*end+1);
                                    return Tokenization::Ok(tk);
                                }
                                None if *strict => return self.invalidate(idx),
                                _ => {
                                    *state = ParserState::Normal(*end+1);
                                    continue;
                                }
                            }
                        }
                        _ if *strict => return self.invalidate(idx),
                        _ => *state = ParserState::Normal(*end+c.len_utf8()),
                    }
                }
                ParserState::TagWhitespace(offset) => {
                    match c {
                        ' ' | '\t' => {
                            *offset += 1;
                        }
                        '"' => *state = ParserState::TagAttrQuoteNameOpen(*offset+1),
                        'a'..='z' | 'A'..='Z' => {
                            *state = ParserState::TagAttrName{
                                start: *offset,
                                end: *offset+1,
                            };
                        }
                        '>' => {
                            let offset = *offset+1;
                            // return Self::finish_token(token, state, attr_name, offset);
                            return self.finish_token_at(offset);
                        }
                        _ if *strict => return self.invalidate(idx),
                        _ => *state = ParserState::Normal(*offset+c.len_utf8()),
                    }
                }
                ParserState::TagAttrName{start, end} => {
                    match c {
                        '=' => {
                            let an = buf[*start..*end].to_owned();
                            *attr_name = Some(an);
                            *state = ParserState::TagAttrAssign(*end+1);
                        }
                        ' ' | '\t' => {
                            let an = buf[*start..*end].to_owned();
                            *attr_name = Some(an);
                            Self::apply_attr(token, attr_name, n_applies);
                            *state = ParserState::TagWhitespace(*end+1);
                        }
                        'a'..='z' | 'A'..='Z' | '0'..='9' => {
                            *end += 1;
                        }
                        '>' => {
                            let an = buf[*start..*end].to_owned();
                            *attr_name = Some(an);
                            Self::apply_attr(token, attr_name, n_applies);
                            let offset = *end+1;
                            return self.finish_token_at(offset);
                        }
                        _ if *strict => return self.invalidate(idx),
                        _ => *state = ParserState::Normal(*end+c.len_utf8()),
                    }
                }
                ParserState::TagAttrQuoteNameOpen(offset) => {
                    match c {
                        '"' => {
                            // 不允许空属性名称
                            log::warn!("empty attribute name found in MXP protocol");
                            if *strict {
                                return self.invalidate(idx);
                            } else {
                                *state = ParserState::Normal(idx+1);
                            }
                        }
                        _ => *state = ParserState::TagAttrQuoteName{start: *offset, end: *offset+1},
                    }
                }
                ParserState::TagAttrQuoteName{start, end} => {
                    match c {
                        '"' => {
                            let an = buf[*start..*end].to_owned();
                            *attr_name = Some(an);
                            *state = ParserState::TagAttrQuoteNameClose(*end+1);
                        }
                        _ => {
                            // 引号包括的文本可包含任意字符
                            // 需注意字符长度可大于1字节
                            *end += c.len_utf8();
                        }
                    }
                }
                ParserState::TagAttrQuoteNameClose(offset) => {
                    match c {
                        '=' => *state = ParserState::TagAttrAssign(*offset+1),
                        ' ' | '\t' => {
                            // 无属性值，直接应用属性名
                            Self::apply_attr(token, attr_name, n_applies);
                            *state = ParserState::TagWhitespace(*offset+1);
                        }
                        '>' => {
                            Self::apply_attr(token, attr_name, n_applies);
                            let offset = *offset+1;
                            return self.finish_token_at(offset);
                        }
                        _ if *strict => return self.invalidate(idx),
                        _ => *state = ParserState::Normal(*offset+c.len_utf8()),
                    }
                }
                ParserState::TagAttrAssign(offset) => {
                    match c {
                        ' ' | '\t' => {
                            Self::apply_attr_value(token, attr_name, "", n_applies);
                            *state = ParserState::TagWhitespace(*offset+1);
                        }
                        '>' => {
                            Self::apply_attr_value(token, attr_name, "", n_applies);
                            let offset = *offset+1;
                            return self.finish_token_at(offset);
                        }
                        '"' => {
                            *state = ParserState::TagAttrQuoteValueOpen(*offset+1);
                        }
                        _ => {
                            *state = ParserState::TagAttrValue{
                                start: *offset,
                                // unicode字符字节数可以大于1
                                end: *offset + c.len_utf8(),
                            }
                        }
                    }
                }
                ParserState::TagAttrValue{start, end} => {
                    match c {
                        ' ' | '\t' => {
                            Self::apply_attr_value(token, attr_name, &buf[*start..*end], n_applies);
                            *state = ParserState::TagWhitespace(*end+1);
                        }
                        '>' => {
                            Self::apply_attr_value(token, attr_name, &buf[*start..*end], n_applies);
                            let offset = *end+1;
                            return self.finish_token_at(offset);
                        }
                        _ => {
                            *end += c.len_utf8();
                        }
                    }
                }
                ParserState::TagAttrQuoteValueOpen(offset) => {
                    match c {
                        '"' => {
                            Self::apply_attr_value(token, attr_name, "", n_applies);
                            *state = ParserState::TagAttrQuoteValueClose(*offset+1);
                        }
                        _ => {
                            *state = ParserState::TagAttrQuoteValue{
                                start: *offset,
                                end: *offset + c.len_utf8(),
                            };
                        }
                    }
                }
                ParserState::TagAttrQuoteValue{start, end} => {
                    match c {
                        '"' => {
                            Self::apply_attr_value(token, attr_name, &buf[*start..*end], n_applies);
                            *state = ParserState::TagAttrQuoteValueClose(*end+1);
                        }
                        _ => {
                            *end += c.len_utf8();
                        }
                    }
                }
                ParserState::TagAttrQuoteValueClose(offset) => {
                    match c {
                        ' ' | '\t' => *state = ParserState::TagWhitespace(*offset+1),
                        '>' => {
                            let offset = *offset+1;
                            return self.finish_token_at(offset);
                        }
                        _ if *strict => return self.invalidate(idx),
                        _ => *state = ParserState::Normal(*offset+c.len_utf8()),
                    }
                }
                ParserState::Esc(offset) => {
                    match c {
                        '[' => *state = ParserState::EscBracket(*offset+1),
                        _ if *strict => return self.invalidate(idx),
                        _ => *state = ParserState::Normal(*offset+c.len_utf8()),
                    }
                }
                ParserState::EscBracket(offset) => {
                    match c {
                        '0'..='9' | ';' => *state = ParserState::CSI{start: *offset, end: *offset+1},
                        'm' => {
                            *state = ParserState::Normal(*offset+1);
                            return Tokenization::Ok(Token::SGR(String::new()));
                        }
                        _ if *strict => return self.invalidate(idx),
                        _ => *state = ParserState::Normal(*offset+c.len_utf8()),
                    }
                }
                ParserState::CSI{start, end} => {
                    match c {
                        '0'..='9' | ';' => *end += 1,
                        'm' => {
                            let tk = Token::SGR(buf[*start..*end].to_owned());
                            *state = ParserState::Normal(*end+1);
                            return Tokenization::Ok(tk);
                        }
                        'z' => {
                            match buf[*start..*end].parse::<u8>() {
                                Ok(n) => {
                                    let md = match n {
                                        0 => Mode::Open,
                                        1 => Mode::Secure,
                                        _ if *strict => {
                                            log::warn!("unhandled mxp mode change: {}", n);
                                            return self.invalidate(idx);
                                        }
                                        _ => {
                                            log::warn!("unhandled mxp mode change: {}", n);
                                            *state = ParserState::Normal(*end+1);
                                            continue;
                                        }
                                    };
                                    *mode = md;
                                    *state = ParserState::Normal(*end+1);
                                    return Tokenization::Ok(Token::MxpMode(md));
                                }
                                Err(_) if *strict => {
                                    log::warn!("invalid sequence of mxp mode: {}", &buf[*start..*end]);
                                    return self.invalidate(idx);
                                }
                                _ => *state = ParserState::Normal(*end+c.len_utf8()),
                            }
                        }
                        _ if *strict => return self.invalidate(idx),
                        _ => *state = ParserState::Normal(*end+c.len_utf8()),
                    }
                }
                ParserState::Amper{start, end} => {
                    if *end > *start + 7 {
                        // max length is 7
                        if *strict {
                            return self.invalidate(idx);
                        } else {
                            *state = ParserState::Normal(*end+1);
                            continue;
                        }
                    }
                    match c {
                        'a'..='z' => *end += 1,
                        ';' => {
                            let ac = match &buf[*start..*end] {
                                "&quot" => '"',
                                "&amp" => '&',
                                "&lt" => '<',
                                "&gt" => '>',
                                "&nbsp" => ' ',
                                _ if *strict => {
                                    log::warn!("unhandled amper escape sequence {}", &buf[*start..*end]);
                                    return self.invalidate(idx);
                                }
                                _ => {
                                    *state = ParserState::Normal(*end+c.len_utf8());
                                    continue;
                                }
                            };
                            *state = ParserState::Normal(*end+1);
                            return Tokenization::Ok(Token::AmperChar(ac));
                        }
                        _ if *strict => return self.invalidate(idx),
                        _ => *state = ParserState::Normal(*end+c.len_utf8()),
                    }
                }
            }
        }
        // buf耗尽，根据当前状态返回相应结果
        match state {
            ParserState::Normal(offset) => {
                debug_assert_eq!(buf.len(), *offset);
                let text = Self::unify_text(buf, idx, *offset);
                self.reset();
                Tokenization::Ok(Token::Text(text))
            }
            _ => Tokenization::Pending,
        }
    }

    fn reset(&mut self) {
        let Self{mode, state, buf, token, attr_name, n_applies, ..} = self;
        *mode = Mode::Open;
        *state = ParserState::Normal(0);
        buf.clear();
        Self::clear_token(token, attr_name, n_applies);
    }

    fn clear_token(token: &mut Option<Token>, attr_name: &mut Option<String>, n_applies: &mut usize) {
        *token = None;
        *attr_name = None;
        *n_applies = 0;
    }

    fn invalidate(&mut self, offset: usize) -> Tokenization {
        let raw = self.buf[offset..].to_owned();
        self.reset();
        Tokenization::Invalid(raw)
    }

    fn finish_token_at(&mut self, offset: usize) -> Tokenization {
        let tk = self.token.take().unwrap();
        self.state = ParserState::Normal(offset);
        let Self{token, attr_name, n_applies, ..} = self;
        Self::clear_token(token, attr_name, n_applies);
        Tokenization::Ok(tk)
    }

    fn unify_text(buf: &str, start: usize, end: usize) -> String {
        let text = &buf[start..end];
        text.replace("\r\0", "")
    }

    fn apply_attr(token: &mut Option<Token>, attr_name: &mut Option<String>, n_applies: &mut usize) {
        if let Some(attr_name) = attr_name.take() {
            if let Some(t) = token.as_mut() {
                if *n_applies == 0 {
                    t.apply_first_attr(&attr_name);
                } else {
                    t.apply_attr(&attr_name.to_uppercase());
                }
                *n_applies += 1;
            }
        }
    }

    fn apply_attr_value(token: &mut Option<Token>, attr_name: &mut Option<String>, attr_value: &str, n_applies: &mut usize) {
        if let Some(attr_name) = attr_name.take() {
            if let Some(t) = token.as_mut() {
                t.apply_attr_value(&attr_name.to_uppercase(), attr_value);
                *n_applies += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strict_mxp_bold() {
        let input = "<B><BOLD><STRONG></B>";
        let mut parser = Tokenizer::strict();
        parser.fill(input);
        assert_eq!(Tokenization::Ok(Token::Bold(true)), parser.next());
        assert_eq!(Tokenization::Ok(Token::Bold(true)), parser.next());
        assert_eq!(Tokenization::Ok(Token::Bold(true)), parser.next());
        assert_eq!(Tokenization::Ok(Token::Bold(false)), parser.next());
    }

    #[test]
    fn test_strict_mxp_italic() {
        let input = "<I><ITALIC><EM></I>";
        let mut parser = Tokenizer::strict();
        parser.fill(input);
        assert_eq!(Tokenization::Ok(Token::Italic(true)), parser.next());
        assert_eq!(Tokenization::Ok(Token::Italic(true)), parser.next());
        assert_eq!(Tokenization::Ok(Token::Italic(true)), parser.next());
        assert_eq!(Tokenization::Ok(Token::Italic(false)), parser.next());
    }

    #[test]
    fn test_strict_mxp_underline() {
        let input = "<U><UNDERLINE></U>";
        let mut parser = Tokenizer::strict();
        parser.fill(input);
        assert_eq!(Tokenization::Ok(Token::Underline(true)), parser.next());
        assert_eq!(Tokenization::Ok(Token::Underline(true)), parser.next());
        assert_eq!(Tokenization::Ok(Token::Underline(false)), parser.next());
    }

    #[test]
    fn test_strict_mxp_strikeout() {
        let input = "<S><STRIKEOUT></S>";
        let mut parser = Tokenizer::strict();
        parser.fill(input);
        assert_eq!(Tokenization::Ok(Token::Strikeout(true)), parser.next());
        assert_eq!(Tokenization::Ok(Token::Strikeout(true)), parser.next());
        assert_eq!(Tokenization::Ok(Token::Strikeout(false)), parser.next());
    }

    #[test]
    fn test_strict_mxp_color() {
        let input = r#"<C><COLOR><COLOR FORE=red BACK="white"><COLOR green></C>"#;
        let mut parser = Tokenizer::strict();
        parser.fill(input);
        assert_eq!(Tokenization::Ok(Token::Color{fg: Color::Gray, bg: None}), parser.next());
        assert_eq!(Tokenization::Ok(Token::Color{fg: Color::Gray, bg: None}), parser.next());
        assert_eq!(Tokenization::Ok(Token::Color{fg: Color::Red, bg: Some(Color::White)}), parser.next());
        assert_eq!(Tokenization::Ok(Token::Color{fg: Color::Green, bg: None}), parser.next());
        assert_eq!(Tokenization::Ok(Token::ColorReset), parser.next());
    }

    #[test]
    fn test_strict_mxp_high() {
        let input = "<H><HIGH></H>";
        let mut parser = Tokenizer::strict();
        parser.fill(input);
        assert_eq!(Tokenization::Ok(Token::High(true)), parser.next());
        assert_eq!(Tokenization::Ok(Token::High(true)), parser.next());
        assert_eq!(Tokenization::Ok(Token::High(false)), parser.next());
    }

    #[test]
    fn test_strict_mxp_font() {
        let input = r#"<FONT><FONT FACE="simsun" SIZE=15><FONT "Courier New">"#;
        let mut parser = Tokenizer::strict();
        parser.fill(input);
        assert_eq!(Tokenization::Ok(Token::new_font()), parser.next());
        assert_eq!(Tokenization::Ok(Token::Font{face: "simsun".to_owned(), size: Some(15), fg: None, bg: None}), parser.next());
        assert_eq!(Tokenization::Ok(Token::Font{face: "Courier New".to_owned(), size: None, fg: None, bg: None}), parser.next());
    }

    #[test]
    fn test_strict_mxp_misc_tokens() {
        let input = r#"<NOBR><P></P><BR><SBR>&nbsp;"#;
        let mut parser = Tokenizer::strict();
        parser.fill(input);
        assert_eq!(Tokenization::Ok(Token::NoBr), parser.next());
        assert_eq!(Tokenization::Ok(Token::P(true)), parser.next());
        assert_eq!(Tokenization::Ok(Token::P(false)), parser.next());
        assert_eq!(Tokenization::Ok(Token::Br), parser.next());
        assert_eq!(Tokenization::Ok(Token::Sbr), parser.next());
        assert_eq!(Tokenization::Ok(Token::AmperChar(' ')), parser.next()); 
    }

    #[test]
    fn test_strict_mxp_a() {
        let input = r#"<A href="pkuxkx.net" hint=click expire=nomore></A>"#;
        let mut parser = Tokenizer::strict();
        parser.fill(input);
        assert_eq!(Tokenization::Ok(Token::A{
            href: "pkuxkx.net".to_owned(),
            hint: "click".to_owned(),
            expire: Some("nomore".to_owned()),
        }), parser.next());
        assert_eq!(Tokenization::Ok(Token::AEnd), parser.next());
    }

    #[test]
    fn test_strict_mxp_headers() {
        let input = r#"<H1><H2><H3><H4><H5><H6></H1>"#;
        let mut parser = Tokenizer::strict();
        parser.fill(input);
        assert_eq!(Tokenization::Ok(Token::Header(1, true)), parser.next());
        assert_eq!(Tokenization::Ok(Token::Header(2, true)), parser.next());
        assert_eq!(Tokenization::Ok(Token::Header(3, true)), parser.next());
        assert_eq!(Tokenization::Ok(Token::Header(4, true)), parser.next());
        assert_eq!(Tokenization::Ok(Token::Header(5, true)), parser.next());
        assert_eq!(Tokenization::Ok(Token::Header(6, true)), parser.next()); 
        assert_eq!(Tokenization::Ok(Token::Header(1, false)), parser.next()); 
    }

    #[test]
    fn test_strict_mxp_send() {
        let input = r#"<SEND href="pkuxkx.net" hint="北大侠客行" prompt expire="nosend"></SEND>"#;
        let mut parser = Tokenizer::strict();
        parser.fill(input);
        assert_eq!(Tokenization::Ok(Token::Send{
            href: "pkuxkx.net".to_owned(),
            hint: "北大侠客行".to_owned(),
            prompt: true,
            expire: Some("nosend".to_owned()),
        }), parser.next());
        assert_eq!(Tokenization::Ok(Token::SendEnd), parser.next());
    }

    #[test]
    fn test_strict_mxp_mode() {
        let input = "\x1b[0z\x1b[1z";
        let mut parser = Tokenizer::strict();
        parser.fill(input);
        assert_eq!(Tokenization::Ok(Token::MxpMode(Mode::Open)), parser.next());
        assert_eq!(Tokenization::Ok(Token::MxpMode(Mode::Secure)), parser.next());
    }

    #[test]
    fn test_strict_mxp_sgr() {
        let input = "\x1b[m\x1b[1;37;44m";
        let mut parser = Tokenizer::strict();
        parser.fill(input);
        assert_eq!(Tokenization::Ok(Token::SGR("".to_owned())), parser.next());
        assert_eq!(Tokenization::Ok(Token::SGR("1;37;44".to_owned())), parser.next());
    }

    #[test]
    fn test_strict_mxp_stream() {
        let mut parser = Tokenizer::strict();
        parser.fill("<SUPPO");
        assert_eq!(Tokenization::Pending, parser.next());
        parser.fill("RT>");
        assert_eq!(Tokenization::Ok(Token::Support), parser.next());
        parser.fill("\x1b[1");
        assert_eq!(Tokenization::Pending, parser.next());
        parser.fill("z<H1");
        assert_eq!(Tokenization::Ok(Token::MxpMode(Mode::Secure)), parser.next());
        assert_eq!(Tokenization::Pending, parser.next());
        parser.fill(">");
        assert_eq!(Tokenization::Ok(Token::Header(1, true)), parser.next());
    }

    #[test]
    fn test_strict_mxp_invalid() {
        let mut parser = Tokenizer::strict();
        parser.fill("<1");
        debug_assert!(parser.next().invalid());
        parser.fill("<x-");
        debug_assert!(parser.next().invalid());
        parser.fill("\x1bN");
        debug_assert!(parser.next().invalid());
        parser.fill("\x1b[1x");
        debug_assert!(parser.next().invalid());
        parser.fill("&toolongsymbol;");
        debug_assert!(parser.next().invalid());
        parser.fill("&nnnn;");
        debug_assert!(parser.next().invalid());
    }
}