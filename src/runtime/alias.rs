use crate::error::Result;
use crate::runtime::{Pattern, Target, Scripts};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

pub struct Alias {
    pub model: AliasModel,
    pattern: Pattern,
    scripts: Scripts,
}

impl Alias {
    pub fn is_match(&self, input: &str) -> bool {
        self.pattern.is_match(input, true)
    }

    pub fn prepare_scripts(&self, input: &str) -> Option<Cow<str>> {
        super::prepare_scripts(&self.pattern, &self.scripts, input)
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
        let (pattern, scripts) = super::compile_scripts(&self.pattern, &self.scripts, self.regexp, 1)?;
        Ok(Alias {
            model: self,
            pattern,
            scripts
        })
    }
}
