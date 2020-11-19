use crate::error::Result;
use crate::runtime::model::{Model, ModelExec, ModelStore};
use crate::runtime::Target;
use bitflags::bitflags;
use serde::{Deserialize, Serialize};

pub type Trigger = ModelExec<TriggerModel>;

pub type Triggers = ModelStore<Trigger>;

bitflags! {
    pub struct TriggerFlags: u16 {
        const Enabled = 0x0001;
        const OmitFromLog = 0x0002;
        const OmitFromOutput = 0x0004;
        const KeepEvaluating = 0x0008;
        const IgnoreCase = 0x10;
        const RegularExpression = 0x0020;
        const ExpandVariables = 0x0200;
        const LowercaseWildcard = 0x0400;
        const Replace = 0x0400;
        const Temporary = 0x4000;
        const OneShot = 0x8000;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TriggerModel {
    pub name: String,
    pub group: String,
    pub pattern: String,
    pub match_lines: u32,
    pub enabled: bool,
}

impl Default for TriggerModel {
    fn default() -> Self {
        Self {
            name: String::new(),
            group: String::from("default"),
            pattern: String::new(),
            match_lines: 1,
            enabled: false,
        }
    }
}

impl Model for TriggerModel {
    fn name(&self) -> &str {
        &self.name
    }

    fn group(&self) -> &str {
        &self.group
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn compile(self) -> Result<Trigger> {
        let re =
            super::compile_pattern(&self.pattern, 1)?;
        Ok(Trigger::new(self, re))
    }
}

impl TriggerModel {
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = group.into();
        self
    }

    pub fn pattern(mut self, pattern: impl Into<String>) -> Self {
        self.pattern = pattern.into();
        self
    }

    pub fn match_lines(mut self, match_lines: u32) -> Self {
        self.match_lines = match_lines;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
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
        let tr = TriggerModel::default()
            .pattern("^你一觉醒来.*")
            .compile()
            .unwrap();
        assert!(tr.is_match(input));
        let tr = TriggerModel::default()
            .pattern("^(.*)一觉(.*)来.*")
            .compile()
            .unwrap();
        assert!(tr.is_match(input));

        let input = "100/200\n300/400";
        let tr = TriggerModel::default()
            .pattern("^(\\d+)/\\d+\n(\\d+)/\\d+$")
            .compile()
            .unwrap();
        assert!(tr.is_match(input));
    }
}
