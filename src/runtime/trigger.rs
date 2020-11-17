use crate::error::Result;
use crate::runtime::{Target};
use crate::runtime::model::{Model, ModelExec, ModelStore};
use serde::{Deserialize, Serialize};
use bitflags::bitflags;

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
    pub regexp: bool,
    pub target: Target,
    pub match_lines: u32,
    pub seq: u32,
    pub enabled: bool,
    pub scripts: String,
}

impl Default for TriggerModel {
    fn default() -> Self {
        Self {
            name: String::new(),
            group: String::from("default"),
            pattern: String::new(),
            regexp: false,
            target: Target::World,
            match_lines: 1,
            seq: 100,
            enabled: false,
            scripts: String::new(),
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

    fn target(&self) -> Target {
        self.target
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn compile(self) -> Result<Trigger> {
        let (pattern, scripts) =
            super::compile_scripts(&self.pattern, &self.scripts, self.regexp, 1)?;
        Ok(Trigger::new(self, pattern, scripts))
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

    pub fn text(mut self, pattern: impl Into<String>) -> Self {
        self.pattern = pattern.into();
        self.regexp = false;
        self
    }

    pub fn regexp(mut self, pattern: impl Into<String>) -> Self {
        self.pattern = pattern.into();
        self.regexp = true;
        self
    }

    pub fn target(mut self, target: Target) -> Self {
        self.target = target;
        self
    }

    pub fn match_lines(mut self, match_lines: u32) -> Self {
        self.match_lines = match_lines;
        self
    }

    pub fn seq(mut self, seq: u32) -> Self {
        self.seq = seq;
        self
    }

    pub fn scripts(mut self, scripts: impl Into<String>) -> Self {
        self.scripts = scripts.into();
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn compile(self) -> Result<Trigger> {
        let (pattern, scripts) = super::compile_scripts(
            &self.pattern,
            &self.scripts,
            self.regexp,
            self.match_lines as usize,
        )?;
        Ok(Trigger::new(self, pattern, scripts))
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
            .regexp("^你一觉醒来.*")
            .scripts("say hi")
            .compile()
            .unwrap();
        assert!(tr.is_match(input));
        assert_eq!("say hi", tr.prepare_scripts(input).unwrap());
        let tr = TriggerModel::default()
            .regexp("^(.*)一觉(.*)来.*")
            .scripts("say %1 %2")
            .compile()
            .unwrap();
        assert!(tr.is_match(input));
        assert_eq!("say 你 醒", tr.prepare_scripts(input).unwrap());

        let input = "100/200\n300/400";
        let tr = TriggerModel::default()
            .regexp("^(\\d+)/\\d+\n(\\d+)/\\d+$")
            .scripts("say %1 %2")
            .compile()
            .unwrap();
        assert!(tr.is_match(input));
        assert_eq!("say 100 300", tr.prepare_scripts(input).unwrap());
    }
}
