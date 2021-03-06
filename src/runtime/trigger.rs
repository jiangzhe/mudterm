use crate::runtime::cache::{CacheText, InlineStyle};
use crate::runtime::model::{MapModelStore, Model, ModelMatch};
use bitflags::bitflags;

pub type Triggers = MapModelStore<Trigger>;

impl Triggers {
    /// 匹配文本，返回匹配成功的触发器列表
    ///
    /// 与match_first不同之处在于支持多行匹配
    pub fn trigger_first(&self, text: &CacheText) -> Option<(&Trigger, String, Vec<InlineStyle>)> {
        self.0.values()
            .find_map(|tr| tr.match_trigger(text))
    }

    pub fn trigger_all(&self, text: &CacheText) -> Vec<(&Trigger, String, Vec<InlineStyle>)> {
        self.0.values()
            .filter_map(|tr| tr.match_trigger(text))
            .collect()
    }
}

pub type Trigger = Model<TriggerExtra>;

impl Trigger {
    // /// 针对多行匹配进行处理
    pub fn match_trigger(&self, text: &CacheText) -> Option<(&Trigger, String, Vec<InlineStyle>)> {
        if self.extra.match_lines > 1 {
            if let Some(multilines) = text.lastn_trimmed(self.extra.match_lines as usize) {
                if self.is_match(multilines) {
                    return Some((self, multilines.to_owned(), vec![]));
                }
            }
        } else {
            if let Some((line, styles)) = text.last_trimmed() {
                if self.is_match(line) {
                    return Some((self, line.to_owned(), styles.to_vec()));
                }
            }
        }
        None
    }
}

bitflags! {
    pub struct TriggerFlags: u16 {
        // const ENABLED = 0x0001;
        // const OmitFromLog = 0x0002;
        // const OmitFromOutput = 0x0004;
        const KEEP_EVALUATING = 0x0008;
        // const IgnoreCase = 0x10;
        // const RegularExpression = 0x0020;
        // const ExpandVariables = 0x0200;
        // const LowercaseWildcard = 0x0400;
        // const Replace = 0x0400;
        // const Temporary = 0x4000;
        const ONESHOT = 0x8000;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TriggerExtra {
    pub match_lines: u8,
    pub flags: TriggerFlags,
}

impl Default for TriggerExtra {
    fn default() -> Self {
        Self{match_lines: 1, flags: TriggerFlags::empty()}
    }
}

impl ModelMatch for Model<TriggerExtra> {
    type Input = str;

    fn is_match(&self, input: &str) -> bool {
        self.re.is_match(input)
    }
}

impl TriggerExtra {
    pub fn one_shot(&self) -> bool {
        self.flags.contains(TriggerFlags::ONESHOT)
    }

    pub fn set_one_shot(&mut self, one_shot: bool) {
        if one_shot {
            self.flags.insert(TriggerFlags::ONESHOT);
        } else {
            self.flags.remove(TriggerFlags::ONESHOT);
        }
    }
}

pub const NO_TRIGGERS: [Trigger; 0] = [];

#[cfg(test)]
mod tests {

    use super::*;
    use regex::Regex;

    #[test]
    fn test_regex_match() {
        let input = "a\nb";
        let re = Regex::new("^a\nb$").unwrap();
        assert!(re.is_match(input));

        let re = Regex::new("(?m)^hello (?P<v1>.*)\nhello (java)$").unwrap();
        let caps = re.captures("hello world\nhello java").unwrap();
        println!("{:?}", &caps);
        for n in re.capture_names() {
            println!("name={:?}", n);
        }
        for c in caps.iter() {
            println!("match={:?}", c);
        }
    }

    #[test]
    fn test_trigger_match() {
        let input = "你一觉醒来觉得精力充沛。";
        let tr = Trigger::builder()
            .name("t1")
            .pattern("^你一觉醒来.*").unwrap()
            .group("default")
            .build();
        assert!(tr.is_match(input));
        let tr = Trigger::builder()
            .name("t2")
            .pattern("^(.*)一觉(.*)来.*").unwrap()
            .group("default")
            .extra(TriggerExtra::default())
            .build();
        assert!(tr.is_match(input));
        let input = "100/200\n300/400";
        let tr = Trigger::builder()
            .name("t3")
            .pattern("^(\\d+)/\\d+\n(\\d+)/\\d+$").unwrap()
            .group("default")
            .extra(TriggerExtra::default())
            .build();
        assert!(tr.is_match(input));
    }
}
