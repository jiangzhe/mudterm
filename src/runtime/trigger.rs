use crate::error::Result;
use crate::runtime::{Pattern, Target};
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Trigger {
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

impl Default for Trigger {
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

impl Trigger {
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

    pub fn compile(&self) -> Result<CompiledTrigger> {
        let pattern = if self.regexp {
            Pattern::Regex(Regex::new(&self.pattern)?)
        } else {
            Pattern::Plain(self.pattern.to_owned())
        };
        Ok(CompiledTrigger {
            name: self.name.to_owned(),
            group: self.group.to_owned(),
            match_lines: self.match_lines,
            pattern,
            target: self.target,
            enabled: self.enabled,
            content: self.scripts.to_owned(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename = "triggers")]
pub struct Triggers(Vec<Trigger>);

#[derive(Debug, Clone)]
pub struct CompiledTrigger {
    pub name: String,
    pub group: String,
    pub match_lines: u32,
    pub pattern: Pattern,
    pub target: Target,
    pub enabled: bool,
    pub content: String,
}

impl CompiledTrigger {
    pub fn is_match(&self, input: &str) -> bool {
        if self.match_lines != 1 {
            return false;
        }
        match self.pattern {
            Pattern::Plain(ref s) => input.contains(s),
            Pattern::Regex(ref re) => re.is_match(input),
        }
    }
}

pub const NO_TRIGGERS: [CompiledTrigger; 0] = [];

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_trigger_serde() {
        let tr = Trigger::default()
            .group("abc")
            .regexp("^hello$")
            .target(Target::Script)
            .match_lines(5)
            .seq(20)
            .scripts("haha\nhoho\n  heihei");

        let trs = Triggers(vec![tr]);

        let s = serde_yaml::to_string(&trs).unwrap();
        println!("to_string={}", s);

        let trs: Triggers = serde_yaml::from_str(&s).unwrap();
        println!("from_string={:?}", trs);
    }
}
