use crate::ui::line::Line;
use crate::ui::style::Style;
use std::collections::VecDeque;

pub const EMPTY_STYLES: [InlineStyle; 0] = [];

/// 触发器上下文
///
/// 多行颜色暂时不支持
#[derive(Debug)]
pub struct CacheText {
    // 存放纯文本
    text: String,
    // 最小行数，也是文本匹配的最大行数
    min_lines: usize,
    // 最大行数，超过则自动缩容
    max_lines: usize,
    // 每行的字节字节长度，分段格式信息以及是否行结束符
    // 如果想选取最后N行，可以从队列尾部向前遍历N个元素，
    // 将字节长度累加，即得到了N行总长度nlen
    // 然后从raw中截取[rawlen-nlen..]即可得到总长
    meta: VecDeque<LineMeta>,
}

impl CacheText {
    pub fn new(min_lines: usize, max_lines: usize) -> Self {
        assert!(min_lines < max_lines);
        let mut ct = CacheText {
            text: String::new(),
            min_lines,
            max_lines,
            meta: VecDeque::new(),
        };
        for _ in 0..min_lines {
            ct.push_line(&Line::fmt_raw(""));
        }
        ct
    }

    fn ended(&self) -> bool {
        self.text.is_empty() || self.text.ends_with('\n')
    }

    pub fn push_line(&mut self, line: &Line) {
        if !self.ended() {
            let mut last_meta = self.meta.back_mut().unwrap();
            for span in line.spans() {
                let is = InlineStyle {
                    offset: last_meta.len,
                    style: span.style,
                };
                last_meta.len += span.content.len();
                last_meta.styles.push(is);
                self.text.push_str(&span.content);
            }
            return;
        }
        let mut meta = LineMeta::new();
        for span in line.spans() {
            let is = InlineStyle {
                offset: meta.len,
                style: span.style,
            };
            meta.len += span.content.len();
            meta.styles.push(is);
            self.text.push_str(&span.content);
        }
        self.meta.push_back(meta);

        if self.meta.len() >= self.max_lines {
            // 当超过max_lines时触发缩容
            let mut len = 0;
            while self.meta.len() > self.min_lines {
                len += self.meta.pop_front().unwrap().len;
            }
            // 截取text
            self.text = self.text.split_off(len);
        }
    }

    // 获取最后N行文本
    pub fn lastn(&self, n: usize) -> Option<&str> {
        if n == 0 || n > self.min_lines {
            return None;
        }
        let nlen: usize = self.meta.iter().rev().take(n).map(|ld| ld.len).sum();
        let rawlen = self.text.len();
        Some(&self.text[rawlen - nlen..])
    }

    pub fn lastn_trimmed(&self, n: usize) -> Option<&str> {
        self.lastn(n).map(|s| {
            if s.ends_with("\r\n") {
                &s[..s.len()-2]
            } else if s.ends_with('\n') {
                &s[..s.len()-1]
            } else {
                s
            }
        })
    }

    // 获取最后一行文本
    pub fn last(&self) -> Option<(&str, &[InlineStyle])> {
        match self.meta.back() {
            None => None,
            Some(LineMeta { len, styles, .. }) => {
                let rawlen = self.text.len();
                Some((&self.text[rawlen - len..], &styles))
            }
        }
    }

    pub fn last_trimmed(&self) -> Option<(&str, &[InlineStyle])> {
        self.last().map(|(line, styles)| {
            let line = if line.ends_with("\r\n") {
                &line[..line.len() - 2]
            } else if line.ends_with('\n') {
                &line[..line.len() - 1]
            } else {
                line
            };
            let styles = if let Some(last_style) = styles.last() {
                if last_style.offset >= line.len() {
                    &styles[..styles.len() - 1]
                } else {
                    styles
                }
            } else {
                styles
            };
            (line, styles)
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
struct LineMeta {
    len: usize,
    styles: Vec<InlineStyle>,
}

impl LineMeta {
    fn new() -> Self {
        Self {
            len: 0,
            styles: vec![],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct InlineStyle {
    pub offset: usize,
    pub style: Style,
}

impl<'lua> mlua::ToLua<'lua> for InlineStyle {
    fn to_lua(self, lua: &'lua mlua::Lua) -> mlua::Result<mlua::Value<'lua>> {
        let table = lua.create_table()?;
        table.set("offset", self.offset)?;
        if let Some(fg) = self.style.fg {
            table.set("fg", fg.description())?;
        }
        if let Some(bg) = self.style.bg {
            table.set("bg", bg.description())?;
        }
        let mut modifier = self.style.add_modifier;
        modifier.remove(self.style.sub_modifier);
        if !modifier.is_empty() {
            table.set("modifier", modifier.bits())?;
        }
        Ok(mlua::Value::Table(table))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;

    #[test]
    fn test_cache_text_last() {
        let mut ct = CacheText::new(2, 4);
        assert_eq!("\r\n", ct.last().unwrap().0);

        ct.push_line(&Line::fmt_raw("张三走了过来。"));
        assert_eq!("张三走了过来。\r\n", ct.last().unwrap().0);
        ct.push_line(&Line::fmt_raw("李四走了过来。"));
        assert_eq!("张三走了过来。\r\n李四走了过来。\r\n", ct.lastn(2).unwrap());
        ct.push_line(&Line::fmt_raw("hp"));
        ct.push_line(&Line::fmt_raw("sk"));
        assert_eq!("hp\r\nsk\r\n", ct.lastn(2).unwrap());
    }

    #[test]
    fn test_cache_text_with_regex() {
        let re = Regex::new("^(.*)走了过来。$").unwrap();
        let mut ct = CacheText::new(2, 4);
        ct.push_line(&Line::fmt_raw("张三走了过来。"));
        let lastline = ct.last_trimmed().unwrap().0;
        assert!(re.is_match(lastline));
    }
}
