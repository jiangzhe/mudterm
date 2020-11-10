use crate::error::Result;
use crate::runtime::{Pattern, Target};
use regex::Regex;
use serde::{Deserialize, Serialize};

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
        let pattern = if self.regexp {
            Pattern::Regex(Regex::new(&self.pattern)?)
        } else {
            Pattern::Plain(self.pattern.to_owned())
        };
        Ok(Trigger {
            model: self,
            pattern,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Trigger {
    pub model: TriggerModel,
    pattern: Pattern,
}

impl Trigger {
    pub fn is_match(&self, input: &str) -> bool {
        // todo: multiline trigger
        match self.pattern {
            Pattern::Plain(ref s) => input.contains(s),
            Pattern::Regex(ref re) => re.is_match(input),
        }
    }
}

pub const NO_TRIGGERS: [Trigger; 0] = [];

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_trigger_match() {

    }
}
