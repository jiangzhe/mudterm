use crate::error::Result;
use crate::runtime::model::{Model, ModelExec, ModelStore};
use serde::{Deserialize, Serialize};
use regex::Regex;

pub type Aliases = ModelStore<Alias>;

pub type Alias = ModelExec<AliasModel>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AliasModel {
    pub name: String,
    pub group: String,
    pub pattern: String,
    pub enabled: bool,
}

impl Default for AliasModel {
    fn default() -> Self {
        Self {
            name: String::new(),
            group: String::from("default"),
            pattern: String::new(),
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

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn compile(self) -> Result<Alias> {
        let re = Regex::new(&self.pattern)?;
        Ok(Alias::new(self, re))
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

    pub fn pattern(mut self, pattern: impl Into<String>) -> Self {
        self.pattern = pattern.into();
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}
