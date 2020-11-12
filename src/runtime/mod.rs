pub mod alias;
pub mod sub;
pub mod timer;
pub mod trigger;

use crate::codec::{Codec, MudCodec};
use crate::conf;
use crate::error::{Error, Result};
use crate::event::{Event, EventQueue, NextStep, RuntimeEvent, RuntimeEventHandler};
use crate::ui::span::ArcSpan;
use crate::ui::line::Line;
use crate::ui::ansi::SpanStream;
use alias::Alias;
use crossbeam_channel::Sender;
use regex::Regex;
use rlua::Lua;
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::borrow::Cow;
use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::hash::Hash;
use std::io::Write;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use sub::{Sub, SubParser};

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
    // reflector: Reflector,
    span_stream: SpanStream,
    echo_cmd: bool,
    cmd_delim: char,
    log_ansi: bool,
    send_empty_cmd: bool,
    cmd_nr: usize,
    logger: Option<File>,
}

impl Runtime {
    pub fn new(evttx: Sender<Event>, config: &conf::Config) -> Self {
        Self {
            evttx,
            lua: Lua::new(),
            queue: EventQueue::new(),
            vars: Variables::new(),
            mud_codec: MudCodec::new(),
            // reflector: Reflector::default(),
            span_stream: SpanStream::new().reserve_cr(config.term.reserve_cr),
            echo_cmd: config.term.echo_cmd,
            cmd_delim: config.term.cmd_delim,
            log_ansi: config.server.log_ansi,
            send_empty_cmd: config.term.send_empty_cmd,
            cmd_nr: 0,
            logger: None,
        }
    }

    pub fn set_logger(&mut self, logger: File) {
        self.logger = Some(logger);
    }

    pub fn exec<T: AsRef<[u8]>>(&self, input: T) -> Result<()> {
        let input = input.as_ref();
        let output = self.lua.context::<_, rlua::Result<()>>(move |lua_ctx| {
            let rst = lua_ctx.load(input).exec()?;
            Ok(rst)
        })?;
        Ok(output)
    }

    pub fn preprocess_user_cmd(&mut self, mut cmd: String) {
        // todo: might be regenerated by alias
        // todo: might be other built-in command
        if cmd.ends_with('\n') {
            cmd.truncate(cmd.len() - 1);
        }
        // todo: add alias/script handling
        let cmds = translate_cmds(cmd, self.cmd_delim, self.send_empty_cmd, &vec![]);
        for (tgt, cmd) in cmds {
            match tgt {
                Target::World => {
                    self.cmd_nr += 1;
                    if self.echo_cmd && self.cmd_nr > 2 {
                        // self.queue.push_styled_line(StyledLine::raw(cmd.to_owned()));
                        self.queue.push_line(Line::raw(cmd.to_owned()));
                    }
                    self.queue.push_cmd(cmd);
                }
                Target::Script => {
                    if let Err(e) = self.exec(cmd) {
                        // self.queue.push_styled_line(StyledLine::err(e.to_string()));
                        self.queue.push_line(Line::err(e.to_string()));
                    }
                }
            }
        }
    }

    pub fn process_user_scripts(&mut self, scripts: String) {
        if let Err(e) = self.exec(&scripts) {
            // self.queue.push_styled_line(StyledLine::err(e.to_string()));
            self.queue.push_line(Line::err(e.to_string()));
        }
    }

    // pub fn process_mud_lines(&mut self, lines: VecDeque<StyledLine>) {
    //     for line in lines {
    //         self.process_mud_line(line);
    //     }
    // }

    // pub fn process_mud_line(&mut self, line: StyledLine) {
    //     // todo: apply triggers here
    //     self.queue.push_styled_line(line);
    // }

    pub fn process_mud_spans(&mut self, spans: Vec<ArcSpan>) {
        for span in spans {
            // handle trigger
            self.queue.push_span(span);
        }
    }

    pub fn process_bytes_from_mud(&mut self, bs: &[u8]) -> Result<()> {
        let s = self.mud_codec.decode(bs, true);
        // log server output with ansi sequence
        if let Some(logger) = self.logger.as_mut() {
            if self.log_ansi {
                logger.write_all(s.as_bytes())?;
            }
        }
        // let sms = self.reflector.reflect(s);
        self.span_stream.fill(s);
        let mut spans = Vec::new();
        while let Some(span) = self.span_stream.next_span() {
            spans.push(span);
        }

        // log server output without ansi sequence
        if let Some(logger) = self.logger.as_mut() {
            if !self.log_ansi {
                for span in &spans {
                    logger.write_all(span.content().as_bytes())?;
                    if span.ended {
                        logger.write_all(b"\n")?;
                    }
                }
            }
        }
        // send to global event bus
        self.evttx.send(Event::SpansFromMud(spans))?;
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
        self.lua.context::<_, rlua::Result<()>>(|lua_ctx| {
            let globals = lua_ctx.globals();

            // 初始化SetVariable函数
            let vars = self.vars.clone();
            let set_variable = lua_ctx.create_function(move |_, (k, v): (String, String)| {
                vars.insert(k, v);
                Ok(())
            })?;
            globals.set("SetVariable", set_variable)?;

            // 初始化GetVariable函数
            let vars = self.vars.clone();
            let get_variable = lua_ctx.create_function(move |_, k: String| match vars.get(&k) {
                Some(v) => Ok(Some(v.to_owned())),
                None => Ok(None),
            })?;
            globals.set("GetVariable", get_variable)?;

            // 初始化SwitchCodec函数
            let queue = self.queue.clone();
            let switch_codec = lua_ctx.create_function(move |_, code: String| {
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

            // 初始化Send函数
            let queue = self.queue.clone();
            let send = lua_ctx.create_function(move |_, mut s: String| {
                eprintln!("Send function called");
                queue.push_cmd(s);
                Ok(())
            })?;
            globals.set("Send", send)?;

            // 初始化Note函数
            let queue = self.queue.clone();
            let note = lua_ctx.create_function(move |_, s: String| {
                eprintln!("Note function called");
                queue.push_line(Line::note(s));
                Ok(())
            })?;
            globals.set("Note", note)?;
            Ok(())
        })?;
        Ok(())
    }
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

pub fn translate_cmds(mut cmd: String, delim: char, send_empty_cmd: bool, aliases: &[Alias]) -> Vec<(Target, String)> {
    if cmd.ends_with('\n') {
        cmd.truncate(cmd.len() - 1);
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
