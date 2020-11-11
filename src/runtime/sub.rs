use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum Sub {
    Text(String),
    Number(u8),
    Name(String),
}

impl Sub {
    pub fn is_text(&self) -> bool {
        match self {
            Sub::Text(_) => true,
            _ => false,
        }
    }

    pub fn as_text(self) -> Option<String> {
        match self {
            Sub::Text(s) => Some(s),
            _ => None,
        }
    }

    pub fn is_number(&self) -> bool {
        match self {
            Sub::Number(_) => true,
            _ => false,
        }
    }

    pub fn as_number(self) -> Option<u8> {
        match self {
            Sub::Number(n) => Some(n),
            _ => None,
        }
    }

    pub fn is_name(&self) -> bool {
        match self {
            Sub::Name(_) => true,
            _ => false,
        }
    }

    pub fn as_name(self) -> Option<String> {
        match self {
            Sub::Name(s) => Some(s),
            _ => None,
        }
    }
}

pub struct SubParser {
    state: SubState,
    buf: String,
}

impl SubParser {
    pub fn new() -> Self {
        SubParser {
            state: SubState::None,
            buf: String::new(),
        }
    }

    pub fn parse(mut self, input: &str) -> Result<Vec<Sub>> {
        let mut rs = Vec::new();
        for c in input.chars() {
            match c {
                '%' if self.state == SubState::None => {
                    if !self.buf.is_empty() {
                        rs.push(Sub::Text(std::mem::replace(&mut self.buf, String::new())));
                    }
                    self.state = SubState::Escape;
                }
                '%' if self.state == SubState::Escape => {
                    // double % means one plain-text %
                    self.buf.push(c);
                    self.state = SubState::None;
                }
                '0'..='9' if self.state == SubState::Escape => {
                    self.buf.push(c);
                    self.state = SubState::Number;
                }
                '0'..='9' if self.state == SubState::Number => {
                    self.buf.push(c);
                }
                '<' if self.state == SubState::Escape => {
                    self.state = SubState::Name;
                }
                '>' if self.state == SubState::Name && !self.buf.is_empty() => {
                    rs.push(Sub::Name(std::mem::replace(&mut self.buf, String::new())));
                    self.state = SubState::None;
                }
                'a'..='z' | 'A'..='Z' | '_' if self.state == SubState::Escape => {
                    self.buf.push(c);
                    self.state = SubState::Name;
                }
                'a'..='z' | 'A'..='Z' | '_' | '0'..='9' if self.state == SubState::Name => {
                    self.buf.push(c);
                }
                _ if self.state == SubState::None => {
                    self.buf.push(c);
                }
                _ if self.state == SubState::Number => match self.buf.parse::<u8>() {
                    Ok(n) => {
                        rs.push(Sub::Number(n));
                        self.state = SubState::None;
                        self.buf.clear();
                        self.buf.push(c);
                    }
                    Err(e) => return Err(Error::CompileScriptError(format!("{} in {}", e, input))),
                },
                _ => {
                    return Err(Error::CompileScriptError(format!(
                        "invalid input {}",
                        input
                    )))
                }
            }
        }
        // only None and Number are valid final state
        match self.state {
            SubState::None => {
                if !self.buf.is_empty() {
                    rs.push(Sub::Text(std::mem::replace(&mut self.buf, String::new())));
                }
            }
            SubState::Number => match self.buf.parse::<u8>() {
                Ok(n) => {
                    rs.push(Sub::Number(n));
                    self.state = SubState::None;
                    self.buf.clear();
                }
                Err(e) => return Err(Error::CompileScriptError(format!("{} in {}", e, input))),
            },
            _ => {
                return Err(Error::CompileScriptError(format!(
                    "invalid input {}",
                    input
                )))
            }
        }
        Ok(rs)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SubState {
    None,
    Escape,
    Number,
    Name,
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_sub_parser() {
        let r = SubParser::new().parse("hello, world").unwrap();
        assert_eq!(vec![Sub::Text("hello, world".into())], r);
        let r = SubParser::new().parse("%1").unwrap();
        assert_eq!(vec![Sub::Number(1)], r);
        let r = SubParser::new().parse("say %1").unwrap();
        assert_eq!(vec![Sub::Text("say ".into()), Sub::Number(1)], r);
        let r = SubParser::new().parse("say %<var1> but %2 to end").unwrap();
        assert_eq!(
            vec![
                Sub::Text("say ".into()),
                Sub::Name("var1".into()),
                Sub::Text(" but ".into()),
                Sub::Number(2),
                Sub::Text(" to end".into())
            ],
            r
        );
    }
}
