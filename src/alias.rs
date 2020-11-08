use serde::{Serialize, Deserialize};
use crate::script::{Pattern, Target};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Alias {
    pub name: String,
    pub group: String,
    pub pattern: String,
    pub regexp: bool,
    pub target: Target,
    pub scripts: String,
    pub seq: u32,
    pub enabled: bool,
}

impl Default for Alias {
    fn default() -> Self {
        Self{
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

