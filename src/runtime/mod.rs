pub mod alias;
pub mod model;
pub mod sub;
pub mod timer;
pub mod trigger;

use crate::codec::{Codec, MudCodec};
use crate::conf;
use crate::error::{Error, Result};
use crate::event::{Event, EventQueue, NextStep, RuntimeEvent, RuntimeEventHandler};
use crate::ui::line::RawLine;
use crate::ui::style::{Color, Style};
use alias::Alias;
use crossbeam_channel::Sender;
use mlua::Lua;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fs::File;
use std::hash::Hash;
use std::io::Write;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use sub::{Sub, SubParser};
use trigger::{Trigger, Triggers};
use uuid::Uuid;

/// 脚本环境中的变量存储和查询
#[derive(Debug, Clone)]
pub struct Variables(Arc<RwLock<HashMap<String, String>>>);

impl Variables {
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(HashMap::new())))
    }

    pub fn get<Q>(&self, name: &Q) -> Option<String>
    where
        String: Borrow<Q>,
        Q: Hash + Eq,
    {
        let m = self.0.read().unwrap();
        m.get(name).map(|s| s.to_owned())
    }

    pub fn insert(&self, name: String, value: String) -> Option<String> {
        let mut m = self.0.write().unwrap();
        m.insert(name, value)
    }
}

/// 运行时保存别名，定时器，触发器，以及脚本引擎
/// 可直接提交事件到总线
pub struct Runtime {
    pub(crate) evttx: Sender<Event>,
    lua: Lua,
    // 允许外部直接调用queue的公共方法
    pub(crate) queue: EventQueue,
    vars: Variables,
    pub(crate) mud_codec: MudCodec,
    echo_cmd: bool,
    cmd_delim: char,
    send_empty_cmd: bool,
    cmd_nr: usize,
    logger: Option<File>,
    triggers: Triggers,
}

impl Runtime {
    pub fn new(evttx: Sender<Event>, config: &conf::Config) -> Self {
        Self {
            evttx,
            lua: Lua::new(),
            queue: EventQueue::new(),
            vars: Variables::new(),
            mud_codec: MudCodec::new(),
            // local buffer for triggers
            // buffer: RawLineBuffer::with_capacity(10),
            echo_cmd: config.term.echo_cmd,
            cmd_delim: config.term.cmd_delim,
            send_empty_cmd: config.term.send_empty_cmd,
            cmd_nr: 0,
            logger: None,
            triggers: Triggers::new(),
        }
    }

    pub fn set_logger(&mut self, logger: File) {
        self.logger = Some(logger);
    }

    pub fn exec<T: AsRef<[u8]>>(&self, input: T) -> Result<()> {
        let input = input.as_ref();
        let output = self.lua.load(input).exec()?;
        Ok(())
    }

    pub fn preprocess_user_cmd(&mut self, mut cmd: String) {
        // todo: might be other built-in command
        if cmd.ends_with("\r\n") {
            cmd.truncate(cmd.len() - 2);
        } else if cmd.ends_with('\n') {
            cmd.truncate(cmd.len() - 1)
        }
        let cmds = translate_cmds(cmd, self.cmd_delim, self.send_empty_cmd, &vec![]);
        for (tgt, cmd) in cmds {
            match tgt {
                Target::World => {
                    self.cmd_nr += 1;
                    if self.echo_cmd && self.cmd_nr > 2 {
                        // self.queue.push_styled_line(StyledLine::raw(cmd.to_owned()));
                        self.queue.push_line(RawLine::fmt_raw(&cmd));
                    }
                    self.queue.push_cmd(cmd);
                }
                Target::Script => {
                    if let Err(e) = self.exec(cmd) {
                        let err = format_err(e.to_string());
                        self.queue.push_line(RawLine::fmt_err(err));
                    }
                }
            }
        }
    }

    pub fn process_user_scripts(&mut self, scripts: String) {
        if let Err(e) = self.exec(&scripts) {
            let err = format_err(e.to_string());
            self.queue.push_line(RawLine::fmt_err(err));
        }
    }

    pub fn process_world_line(&mut self, line: RawLine) {
        // todo: apply triggers here
        self.queue.push_line(line);
    }

    pub fn process_world_lines(&mut self, lines: impl IntoIterator<Item = RawLine>) {
        for line in lines {
            self.process_world_line(line);
        }
    }

    pub fn process_bytes_from_mud(&mut self, bs: &[u8]) -> Result<()> {
        let s = self.mud_codec.decode(bs);
        let s: Arc<str> = Arc::from(s);
        // log server output with ansi sequence
        if let Some(logger) = self.logger.as_mut() {
            logger.write_all(s.as_ref().as_bytes())?;
        }

        // here just split into lines
        let mut lines = Vec::new();
        let mut start = 0usize;
        while let Some(end) = s.as_ref()[start..].find('\n') {
            let end = start + end;
            let line = RawLine::new(s[start..end + 1].to_owned());
            lines.push(line);
            start = end + 1;
        }
        if start < s.as_ref().len() {
            let line = RawLine::new(s.as_ref()[start..].to_owned());
            lines.push(line);
        }
        // send to global event bus
        self.evttx.send(Event::WorldLines(lines))?;
        Ok(())
    }

    pub fn process_runtime_events<H: RuntimeEventHandler>(
        &mut self,
        handler: &mut H,
    ) -> Result<NextStep> {
        let evts = self.queue.drain_all();
        for evt in evts {
            match handler.on_runtime_event(evt, self)? {
                NextStep::Quit => return Ok(NextStep::Quit),
                NextStep::Skip => return Ok(NextStep::Skip),
                _ => (),
            }
        }
        Ok(NextStep::Run)
    }

    pub fn init(&self) -> Result<()> {
        let globals = self.lua.globals();

        // 初始化SetVariable函数
        let vars = self.vars.clone();
        let set_variable = self
            .lua
            .create_function(move |_, (k, v): (String, String)| {
                log::trace!("SetVariable function called");
                vars.insert(k, v);
                Ok(())
            })?;
        globals.set("SetVariable", set_variable)?;

        // 初始化GetVariable函数
        let vars = self.vars.clone();
        let get_variable = self.lua.create_function(move |_, k: String| {
            log::trace!("GetVariable function called");
            match vars.get(&k) {
                Some(v) => Ok(Some(v.to_owned())),
                None => Ok(None),
            }
        })?;
        globals.set("GetVariable", get_variable)?;

        // 初始化SwitchCodec函数
        let queue = self.queue.clone();
        let switch_codec = self.lua.create_function(move |_, code: String| {
            log::trace!("SwitchCodec function called");
            let new_code = match &code.to_lowercase()[..] {
                "gbk" => Codec::Gb18030,
                "utf8" | "utf-8" => Codec::Utf8,
                "big5" => Codec::Big5,
                _ => return Ok(()),
            };
            queue.push(RuntimeEvent::SwitchCodec(new_code));
            Ok(())
        })?;
        globals.set("SwitchCodec", switch_codec)?;

        // 重写print函数
        let print = self.lua.create_function(move |_, s: ()| Ok(()))?;
        globals.set("print", print)?;

        // 初始化Send函数
        let queue = self.queue.clone();
        let send = self.lua.create_function(move |_, s: String| {
            log::trace!("Send function called");
            queue.push_cmd(s);
            Ok(())
        })?;
        globals.set("Send", send)?;

        // 初始化Note函数
        let queue = self.queue.clone();
        let note = self.lua.create_function(move |_, s: String| {
            log::trace!("Note function called");
            queue.push_line(RawLine::fmt_note(s));
            Ok(())
        })?;
        globals.set("Note", note)?;

        // 初始化ColourNote函数
        let queue = self.queue.clone();
        let colour_note =
            self.lua
                .create_function(move |_, (fg, bg, text): (String, String, String)| {
                    log::trace!("ColourNote function called");
                    let style = Style::default()
                        .fg(Color::from_str_or_default(fg, Color::Reset))
                        .bg(Color::from_str_or_default(bg, Color::Reset));
                    queue.push_line(RawLine::fmt(text, style));
                    Ok(())
                })?;
        globals.set("ColourNote", colour_note)?;

        let get_unique_id = self.lua.create_function(move |_, _: ()| {
            let id = Uuid::new_v4();
            Ok(id.to_simple().to_string())
        })?;
        globals.set("GetUniqueID", get_unique_id)?;
        // todo: 初始化AddTrigger函数
        Ok(())
    }
}

fn format_err(err: impl AsRef<str>) -> String {
    let err = err.as_ref();
    let err = if err.ends_with("\r\n") {
        &err[..err.len() - 2]
    } else if err.ends_with('\n') {
        &err[..err.len() - 1]
    } else {
        &err
    };
    let mut s = String::new();
    for p in err.split('\n') {
        s.push_str(p);
        if !p.ends_with('\r') {
            s.push_str("\r\n");
        } else {
            s.push('\n');
        }
    }
    s
}

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
            Pattern::Regex(re) => re.is_match(input),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Scripts {
    Plain(String),
    Subs(Vec<Sub>),
}

impl FromStr for Scripts {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        if s.is_empty() {
            return Ok(Scripts::Plain(String::new()));
        }
        let mut subs = SubParser::new().parse(s)?;
        if subs.len() == 1 && subs[0].is_text() {
            return Ok(Scripts::Plain(subs.pop().unwrap().as_text().unwrap()));
        }
        Ok(Scripts::Subs(subs))
    }
}

/// 编译匹配及运行脚本
fn compile_scripts(
    pattern: &str,
    scripts: &str,
    regexp: bool,
    match_lines: usize,
) -> Result<(Pattern, Scripts)> {
    if pattern.is_empty() {
        return Err(Error::RuntimeError(
            "The match_text cannot be empty".to_owned(),
        ));
    }
    if regexp {
        // handle multi-line
        let re = if match_lines > 1 {
            let mut pat = String::with_capacity(pattern.len() + 4);
            // enable multi-line feature by prefix 'm' flag
            pat.push_str("(?m)");
            pat.push_str(&pattern);
            Regex::new(&pat)?
        } else {
            Regex::new(&pattern)?
        };
        Ok((Pattern::Regex(re), scripts.parse()?))
    } else {
        Ok((
            Pattern::Plain(pattern.to_owned()),
            Scripts::Plain(scripts.to_owned()),
        ))
    }
}

/// 正则匹配并替换脚本中的占位符
///
/// 若输入无法匹配正则，返回空
fn prepare_scripts<'a>(
    pattern: &Pattern,
    scripts: &'a Scripts,
    input: &str,
) -> Option<Cow<'a, str>> {
    match (pattern, scripts) {
        (_, Scripts::Plain(s)) => Some(Cow::Borrowed(s)),
        (Pattern::Regex(re), Scripts::Subs(subs)) => {
            if let Some(caps) = re.captures(input) {
                let mut r = String::new();
                for sub in subs {
                    match sub {
                        Sub::Text(s) => r.push_str(s),
                        Sub::Number(num) => {
                            if let Some(m) = caps.get(*num as usize) {
                                r.push_str(m.as_str());
                            }
                        }
                        Sub::Name(name) => {
                            if let Some(m) = caps.name(name) {
                                r.push_str(m.as_str());
                            }
                        }
                    }
                }
                Some(Cow::Owned(r))
            } else {
                return None;
            }
        }
        _ => unreachable!("plain pattern with subs scripts"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Target {
    World,
    Script,
}

pub fn translate_cmds(
    cmd: String,
    delim: char,
    send_empty_cmd: bool,
    aliases: &[Alias],
) -> Vec<(Target, String)> {
    if cmd.is_empty() {
        return vec![(Target::World, String::new())];
    }
    let raw_lines: Vec<String> = cmd
        .split(|c| c == '\n' || c == delim)
        .filter(|s| send_empty_cmd || !s.is_empty())
        .map(|s| s.to_owned())
        .collect();
    let mut cmds = Vec::new();
    for raw_line in raw_lines {
        if raw_line.is_empty() {
            // send empty line directly, maybe filtered before this action
            cmds.push((Target::World, raw_line));
        } else if let Some(alias) = match_aliases(&raw_line, aliases) {
            log::debug!(
                "alias[{}/{}: {}] matched",
                alias.model.group,
                alias.model.name,
                alias.model.pattern
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
