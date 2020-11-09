use crate::error::Result;
use crate::runtime::{Pattern, Target};
use regex::Regex;
use serde::{Deserialize, Serialize};

pub struct Alias {
    pub model: AliasModel,
    pattern: Pattern,
}

impl Alias {
    pub fn is_match(&self, input: &str) -> bool {
        self.pattern.is_match(input, true)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AliasModel {
    pub name: String,
    pub group: String,
    pub pattern: String,
    pub regexp: bool,
    pub target: Target,
    pub scripts: String,
    pub seq: u32,
    pub enabled: bool,
}

impl Default for AliasModel {
    fn default() -> Self {
        Self {
            name: String::new(),
            group: String::from("default"),
            pattern: String::new(),
            regexp: false,
            target: Target::World,
            scripts: String::new(),
            seq: 100,
            enabled: true,
        }
    }
}

impl AliasModel {
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

    pub fn compile(self) -> Result<Alias> {
        let pattern = if self.regexp {
            Pattern::Regex(Regex::new(&self.pattern)?)
        } else {
            Pattern::Plain(self.pattern.clone())
        };
        Ok(Alias {
            model: self,
            pattern,
        })
    }
}
