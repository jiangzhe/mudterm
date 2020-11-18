use crate::error::Result;
use crate::runtime::model::{Model, ModelExec, ModelStore};
use crate::runtime::{Pattern, Scripts, Target};
use serde::{Deserialize, Serialize};

pub type Aliases = ModelStore<AliasModel>;

pub type Alias = ModelExec<AliasModel>;

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

impl Model for AliasModel {
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

    fn compile(self) -> Result<Alias> {
        let (pattern, scripts) =
            super::compile_scripts(&self.pattern, &self.scripts, self.regexp, 1)?;
        Ok(Alias::new(self, pattern, scripts))
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
}
