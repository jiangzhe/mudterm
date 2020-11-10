use crate::error::{Result, Error};
use crate::runtime::{Pattern, Target, Scripts};
use crate::runtime::Sub;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::borrow::Cow;

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
        let (pattern, scripts) = if self.regexp {
            // handle multi-line
            let re = if self.match_lines > 1 {
                let mut pat = String::with_capacity(self.pattern.len() + 4);
                // enable multi-line feature by prefix 'm' flag
                pat.push_str("(?m)");
                pat.push_str(&self.pattern);
                Regex::new(&pat)?
            } else {
                Regex::new(&self.pattern)?
            };
            (Pattern::Regex(re), self.scripts.parse()?)
        } else {
            (Pattern::Plain(self.pattern.to_owned()), Scripts::Plain(self.scripts.to_owned()))
        };
        Ok(Trigger {
            model: self,
            pattern,
            scripts,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Trigger {
    pub model: TriggerModel,
    pattern: Pattern,
    scripts: Scripts,
}

impl Trigger {
    pub fn is_match(&self, input: &str) -> bool {
        match &self.pattern {
            Pattern::Plain(s) => input.contains(s),
            Pattern::Regex(re) => re.is_match(input),
        }
    }

    // this method should be called after is_match returns true
    // otherwise, returns none
    pub fn prepared_scripts(&self, input: &str) -> Option<Cow<str>> {
        match (&self.pattern, &self.scripts) {
            (_, Scripts::Plain(s)) => Some(Cow::Borrowed(s)),
            (Pattern::Regex(re), Scripts::Subs(subs)) => {
                if let Some(caps) = re.captures(input) {
                    let mut r = String::new();
                    for sub in subs {
                        match sub {
                            Sub::Text(s) => r.push_str(s),
                            Sub::Number(num) => if let Some(m) = caps.get(*num as usize) {
                                r.push_str(m.as_str());
                            }
                            Sub::Name(name) => if let Some(m) = caps.name(name) {
                                r.push_str(m.as_str());
                            }
                        }
                    }
                    Some(Cow::Owned(r))
                } else {
                    return None;
                }
            },
            _ => unreachable!("plain pattern with subs scripts")
        }
    }
}

pub const NO_TRIGGERS: [Trigger; 0] = [];

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_regex_match() {
        let input = "a\nb";
        let re = Regex::new("^a\nb$").unwrap();
        assert!(re.is_match(input));
    }

    #[test]
    fn test_trigger_match() {
        let input = "你一觉醒来觉得精力充沛。";
        let tr = TriggerModel::default().regexp("^你一觉醒来.*").scripts("say hi").compile().unwrap();
        assert!(tr.is_match(input));
        assert_eq!("say hi", tr.prepared_scripts(input).unwrap());
        let tr = TriggerModel::default().regexp("^(.*)一觉(.*)来.*").scripts("say %1 %2").compile().unwrap();
        assert!(tr.is_match(input));
        assert_eq!("say 你 醒", tr.prepared_scripts(input).unwrap());
        
        let input = "100/200\n300/400";
        let tr = TriggerModel::default().regexp("^(\\d+)/\\d+\n(\\d+)/\\d+$").scripts("say %1 %2").compile().unwrap();
        assert!(tr.is_match(input));
        assert_eq!("say 100 300", tr.prepared_scripts(input).unwrap());
    }
}
