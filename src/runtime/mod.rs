pub mod alias;
pub mod script;
pub mod trigger;

use alias::Alias;
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub enum Pattern {
    Plain(String),
    Regex(Regex),
}

impl Pattern {
    pub fn is_match(&self, input: &str, strict: bool) -> bool {
        match self {
            Pattern::Plain(ref s) => {
                if strict {
                    input == s
                } else {
                    input.contains(s)
                }
            }
            Pattern::Regex(ref re) => re.is_match(input),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Target {
    World,
    Script,
}

pub fn translate_cmds(mut cmd: String, delim: char, aliases: &[Alias]) -> Vec<(Target, String)> {
    if cmd.ends_with('\n') {
        cmd.truncate(cmd.len() - 1);
    }
    let raw_lines: Vec<String> = cmd
        .split(|c| c == '\n' || c == delim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_owned())
        .collect();
    let mut cmds = Vec::new();
    for raw_line in raw_lines {
        if let Some(alias) = match_aliases(&raw_line, aliases) {
            eprintln!(
                "alias[{}/{}: {}] matched",
                alias.model.group, alias.model.name, alias.model.pattern
            );
            cmds.push((alias.model.target, alias.model.scripts.clone()))
        } else {
            cmds.push((Target::World, raw_line))
        }
    }
    cmds
}

fn match_aliases<'a>(input: &str, aliases: &'a [Alias]) -> Option<&'a Alias> {
    for alias in aliases {
        if alias.is_match(&input) {
            return Some(alias);
        }
    }
    None
}
