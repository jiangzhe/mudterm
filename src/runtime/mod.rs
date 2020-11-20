pub mod alias;
pub mod model;
pub mod queue;
pub mod sub;
pub mod timer;
pub mod trigger;
pub mod cache;

use crate::codec::{Codec, MudCodec};
use crate::conf;
use crate::error::{Error, Result};
use crate::event::{Event, NextStep};
use crate::ui::line::{RawLine, RawLines};
use crate::ui::style::{Color, Style};
use crate::ui::UserOutput;
use alias::{Alias, AliasFlags, AliasModel, Aliases};
use crossbeam_channel::Sender;
use mlua::Lua;
use queue::OutputQueue;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::collections::HashMap;
use std::fs::File;
use std::hash::Hash;
use std::io::Write;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use sub::{Sub, SubParser};
use uuid::Uuid;

/// 运行时事件，进由运行时进行处理
///
/// 这些事件是由外部事件派生或由脚本运行生成出来
#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeEvent {
    Output(RuntimeOutput),
    /// 运行时操作
    ImmediateAction(RuntimeAction),
}

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeOutput {
    /// 发送给服务器的命令
    ToServer(String),
    /// 显示在终端上的文本
    ToUI(RawLines),
}

/// 运行时事件回调
pub trait RuntimeOutputHandler {
    fn on_runtime_output(&mut self, output: RuntimeOutput, rt: &mut Runtime) -> Result<NextStep>;
}

/// 运行时操作
#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeAction {
    SwitchCodec(Codec),
    CreateAlias(Alias),
    DeleteAlias(String),
}

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

// 别名回调存储于Lua脚本引擎的全局变量表中
const GLOBAL_ALIAS_CALLBACKS: &str = "_global_alias_callbacks";
// 触发器回调存储于Lua脚本引擎的全局变量表中
const GLOBAL_TRIGGER_CALLBACKS: &str = "_global_trigger_callbacks";
// 计时器回调存储于Lua脚本引擎的全局变量表中
const GLOBAL_TIMER_CALLBACKS: &str = "_global_timer_callbacks";

/// 运行时保存别名，定时器，触发器，以及脚本引擎
/// 可直接提交事件到总线
pub struct Runtime {
    pub(crate) evttx: Sender<Event>,
    lua: Lua,
    // 允许外部直接调用queue的公共方法
    queue: OutputQueue,
    vars: Variables,
    mud_codec: MudCodec,
    cmd_delim: char,
    send_empty_cmd: bool,
    logger: Option<File>,
    // triggers: Triggers,
    aliases: Aliases,
}

impl Runtime {
    /// 创建运行时
    pub fn new(evttx: Sender<Event>, config: &conf::Config) -> Self {
        Self {
            evttx,
            lua: Lua::new(),
            queue: OutputQueue::new(),
            vars: Variables::new(),
            mud_codec: MudCodec::new(),
            // local buffer for triggers
            // buffer: RawLineBuffer::with_capacity(10),
            cmd_delim: config.term.cmd_delim,
            send_empty_cmd: config.term.send_empty_cmd,
            logger: None,
            // triggers: Triggers::new(),
            aliases: Aliases::new(),
        }
    }

    /// 设置日志输出
    pub fn set_logger(&mut self, logger: File) {
        self.logger = Some(logger);
    }

    // 向UI线程推送文字
    pub fn push_line_to_ui(&mut self, line: RawLine) {
        self.queue.push_line(line);
    }

    /// 将UTF-8字符串编码为MUD服务器编码（如GBK，Big5或仍然为UTF-8）
    pub fn encode(&mut self, s: impl AsRef<str>) -> Result<Vec<u8>> {
        self.mud_codec.encode(s.as_ref())
    }

    /// 执行任意脚本，用户可通过UI界面直接输入脚本
    pub fn exec_script(&self, input: impl AsRef<[u8]>) -> Result<()> {
        let input = input.as_ref();
        self.lua.load(input).exec()?;
        Ok(())
    }

    /// 执行别名回调
    pub fn exec_alias(&self, name: String, text: String) -> Result<()> {
        let alias = self.aliases.get(&name).ok_or_else(|| {
            Error::RuntimeError(format!("alias '{}' mismatch with text '{}'", &name, &text))
        })?;
        let wildcards = alias.captures(&text)?;
        let callbacks: mlua::Table = self.lua.globals().get(GLOBAL_ALIAS_CALLBACKS)?;
        let func: mlua::Function = callbacks.get(&alias.model.name[..])?;
        func.call((name, text, wildcards))?;
        Ok(())
    }

    /// 创建别名
    fn create_alias(&mut self, alias: Alias) -> std::result::Result<(), Alias> {
        self.aliases.add(alias)
    }

    /// 删除别名回调：回调注册在Lua全局回调表中
    fn delete_alias_callback(&mut self, name: &str) -> Result<()> {
        let alias_callbacks: mlua::Table = self.lua.globals().get(GLOBAL_ALIAS_CALLBACKS)?;
        alias_callbacks.set(name, mlua::Value::Nil)?;
        Ok(())
    }

    // 删除别名
    fn delete_alias(&mut self, name: &str) -> Result<()> {
        self.delete_alias_callback(name)?;
        self.aliases.remove(name);
        Ok(())
    }

    /// 处理用户输出：命名参考MUSHClient
    pub fn process_user_output(&mut self, output: UserOutput) {
        match output {
            UserOutput::Script(script) => self.process_user_script(script),
            UserOutput::Cmd(cmd) => self.process_user_cmd(cmd),
        }
    }

    /// 处理用户命令，拆分并做别名转换
    fn process_user_cmd(&mut self, mut cmd: String) {
        // todo: might be other built-in command
        if cmd.ends_with("\r\n") {
            cmd.truncate(cmd.len() - 2);
        } else if cmd.ends_with('\n') {
            cmd.truncate(cmd.len() - 1);
        }
        let cmds = self.translate_cmds(cmd, self.cmd_delim, self.send_empty_cmd);
        if cmds.is_empty() {
            // 对于空字符，推送空行
            self.queue.push_cmd("\n".to_owned());
            return;
        }
        for cmd in cmds {
            match cmd {
                PostCmd::Raw(mut s) => {
                    if !s.ends_with('\n') {
                        s.push('\n');
                    }
                    self.queue.push_cmd(s);
                }
                PostCmd::Alias { name, text } => {
                    if let Err(e) = self.exec_alias(name, text) {
                        let err = format_err(e.to_string());
                        self.queue.push_line(RawLine::fmt_err(err));
                    }
                }
            }
        }
    }

    /// 处理用户脚本
    fn process_user_script(&mut self, script: String) {
        if let Err(e) = self.exec_script(&script) {
            let err = format_err(e.to_string());
            self.queue.push_line(RawLine::fmt_err(err));
        }
    }

    pub fn process_world_line(&mut self, line: RawLine) {
        // todo: 需要提前处理文字格式，以便于触发器使用
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

    /// 处理队列中的事件：循环处理直到队列中不再存在操作型事件
    pub fn process_queue(&mut self) -> Vec<RuntimeOutput> {
        let mut iter_count = 50;
        while iter_count > 0 {
            let mut action_exists = false;
            let queued = self.queue.drain_all();
            for output in queued {
                match output {
                    RuntimeEvent::ImmediateAction(action) => {
                        action_exists = true;
                        self.process_runtime_action(action);
                    }
                    other => self.queue.push(other),
                }
            }
            if !action_exists {
                return self
                    .queue
                    .drain_all()
                    .into_iter()
                    .filter_map(|e| match e {
                        RuntimeEvent::Output(output) => Some(output),
                        _ => None,
                    })
                    .collect();
            }
            iter_count -= 1;
        }
        // todo: 需要处理优雅关闭（UI提示用户进行主动关闭）
        log::error!("exceeds limit of 50 iterations on runtime event queue");
        self.evttx.send(Event::Quit).unwrap();
        vec![]
    }

    /// 处理操作型事件
    fn process_runtime_action(&mut self, action: RuntimeAction) {
        match action {
            RuntimeAction::SwitchCodec(code) => {
                self.mud_codec.switch_codec(code);
            }
            RuntimeAction::CreateAlias(alias) => {
                let name = alias.model.name.to_owned();
                if let Err(alias) = self.create_alias(alias) {
                    self.push_line_to_ui(RawLine::fmt_err(format!("创建别名失败：{:?}", alias)));
                    // 注销回调函数，忽略错误
                    if let Err(e) = self.delete_alias_callback(&name) {
                        log::warn!("delete alias callback error {}", e);
                    }
                }
            }
            RuntimeAction::DeleteAlias(name) => {
                if let Err(e) = self.delete_alias(&name) {
                    log::warn!("delete alias error {}", e);
                }
            }
        }
    }

    /// 改写命令，根据换行与分隔符切分命名，并进行别名匹配与替换
    fn translate_cmds(&self, cmd: String, delim: char, send_empty_cmd: bool) -> Vec<PostCmd> {
        if cmd.is_empty() {
            return vec![];
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
                cmds.push(PostCmd::Raw(raw_line));
            } else if let Some(alias) = self.aliases.match_first(&raw_line) {
                log::debug!(
                    "alias[{}/{}: {}] matched",
                    alias.model.group,
                    alias.model.name,
                    alias.model.pattern
                );
                cmds.push(PostCmd::Alias {
                    name: alias.model.name.clone(),
                    text: raw_line,
                })
            } else {
                cmds.push(PostCmd::Raw(raw_line));
            }
        }
        cmds
    }

    /// 初始化运行时
    ///
    /// 1. 定义全局变量表，Lua脚本通过SetVariable()和GetVariable()函数
    ///    对其中的值进行设置和查询
    /// 2. 定义Lua脚本引擎中的的核心函数
    ///    有一部分函数借鉴了MUSHClient的函数签名。
    ///    输入输出函数：Send()，Note()和ColourNote()。
    ///    别名相关函数：CreateAlias(), DeleteAlias(), EnableAlias(), DisableAlias()。
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
            queue.push(RuntimeEvent::ImmediateAction(RuntimeAction::SwitchCodec(
                new_code,
            )));
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

        // 初始化GetUniqueID函数
        let get_unique_id = self.lua.create_function(move |_, _: ()| {
            let id = Uuid::new_v4();
            Ok(id.to_simple().to_string())
        })?;
        globals.set("GetUniqueID", get_unique_id)?;

        // 别名常量
        let alias_flag: mlua::Table = self.lua.create_table()?;
        alias_flag.set("Enabled", 1)?;
        alias_flag.set("KeepEvaluating", 8)?;
        globals.set("alias_flag", alias_flag)?;

        // 别名回调注册表
        let alias_callbacks = self.lua.create_table()?;
        self.lua
            .globals()
            .set(GLOBAL_ALIAS_CALLBACKS, alias_callbacks)?;

        // 初始化CreateAlias函数
        let queue = self.queue.clone();
        let create_alias = self.lua.create_function(
            move |lua,
                  (name, group, pattern, flags, func): (
                String,
                String,
                String,
                u16,
                mlua::Function,
            )| {
                if pattern.is_empty() {
                    return Err(mlua::Error::external(Error::RuntimeError(
                        "empty pattern not allowed when creating alias".to_owned(),
                    )));
                }
                let flags = AliasFlags::from_bits(flags).ok_or_else(|| {
                    mlua::Error::external(Error::RuntimeError(format!(
                        "invalid alias flags {}",
                        flags
                    )))
                })?;

                let alias_callbacks: mlua::Table = lua.globals().get(GLOBAL_ALIAS_CALLBACKS)?;
                if alias_callbacks.contains_key(name.to_owned())? {
                    return Err(mlua::Error::external(Error::RuntimeError(format!(
                        "alias callback '{}' already exists",
                        &name
                    ))));
                }
                let model = AliasModel {
                    name,
                    group,
                    pattern,
                    extra: flags,
                };
                let alias = model.compile().map_err(|e| mlua::Error::external(e))?;
                // 此处可以直接向Lua表中添加回调，因为：
                // 1. mlua::Function与Lua运行时同生命周期，不能在EventQueue中传递
                // 2. 需保证在下次检验名称时若重名则失败
                // 重要：在处理RuntimAction时，需清理曾添加的回调函数
                alias_callbacks.set(alias.model.name.to_owned(), func)?;
                queue.push(RuntimeEvent::ImmediateAction(RuntimeAction::CreateAlias(
                    alias,
                )));
                Ok(())
            },
        )?;
        globals.set("CreateAlias", create_alias)?;

        // 初始化DeleteAlias函数
        let queue = self.queue.clone();
        let delete_alias = self.lua.create_function(move |_, (name,): (String,)| {
            queue.push(RuntimeEvent::ImmediateAction(RuntimeAction::DeleteAlias(
                name,
            )));
            Ok(())
        })?;
        globals.set("DeleteAlias", delete_alias)?;
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
fn compile_pattern(pattern: &str, match_lines: usize) -> Result<Regex> {
    if pattern.is_empty() {
        return Err(Error::RuntimeError(
            "The match_text cannot be empty".to_owned(),
        ));
    }
    let re = if match_lines > 1 {
        let mut pat = String::with_capacity(pattern.len() + 4);
        // enable multi-line feature by prefix 'm' flag
        pat.push_str("(?m)");
        pat.push_str(&pattern);
        Regex::new(&pat)?
    } else {
        Regex::new(&pattern)?
    };
    Ok(re)
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Target {
    World,
    Script,
}

/// 预处理后的命令，用户原始命令，或经过别名匹配后的脚本名
#[derive(Debug, Clone)]
pub enum PostCmd {
    Raw(String),
    Alias { name: String, text: String },
}

// fn match_aliases<'a>(input: &str, aliases: &'a [Alias]) -> Option<&'a Alias> {
//     for alias in aliases {
//         if alias.model.enabled() && alias.is_match(&input) {
//             return Some(alias);
//         }
//     }
//     None
// }

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::{unbounded, Receiver};

    #[test]
    fn test_process_single_user_cmd() {
        let (mut rt, _) = new_runtime().unwrap();
        rt.process_user_output(UserOutput::Cmd("hp".to_owned()));
        assert_eq!(1, rt.queue.len());
        let evt = rt.queue.drain_all().pop().unwrap();
        assert_eq_str("hp\n", evt);
        rt.process_user_output(UserOutput::Cmd("hp\nsay hi".to_owned()));
    }

    #[test]
    fn test_process_multi_user_cmds() {
        let (mut rt, _) = new_runtime().unwrap();
        rt.process_user_output(UserOutput::Cmd("hp;say hi".to_owned()));
        assert_eq!(1, rt.queue.len());
        let mut evts = rt.queue.drain_all();
        assert_eq_str("hp\nsay hi\n", evts.pop().unwrap());

        rt.process_user_output(UserOutput::Cmd("hp\nsay hi".to_owned()));
        assert_eq!(1, rt.queue.len());
        let mut evts = rt.queue.drain_all();
        assert_eq_str("hp\nsay hi\n", evts.pop().unwrap());
    }

    #[test]
    fn test_process_simple_alias() {
        let (mut rt, _) = new_runtime().unwrap();
        rt.lua.load(r#"
        local n = function() Send("north") end
        CreateAlias("alias-n", "map", "^n$", alias_flag.Enabled, n)
        "#).exec().unwrap();
        rt.process_queue();
        // start user output
        rt.process_user_output(UserOutput::Cmd("n".to_owned()));
        let mut outputs = rt.process_queue();
        assert_eq!(1, outputs.len());
        assert_eq!(RuntimeOutput::ToServer("north\n".to_owned()), outputs.pop().unwrap());
    }

    #[test]
    fn test_process_complex_alias() {
        let (mut rt, _) = new_runtime().unwrap();
        rt.lua.load(r#"
        local m = function(name, line, wildcards) Send(wildcards[1]..wildcards[2]) end
        CreateAlias("alias-m", "number", "^num (\\d+)\\s+(\\d+)$", alias_flag.Enabled, m) 
        "#).exec().unwrap();
        rt.process_queue();
        rt.process_user_output(UserOutput::Cmd("x;num 123 456".to_owned()));
        let mut outputs = rt.process_queue();
        assert_eq!(1, outputs.len());
        assert_eq!(RuntimeOutput::ToServer("x\n123456\n".to_owned()), outputs.pop().unwrap());
    }

    fn assert_eq_str(expect: impl AsRef<str>, actual: RuntimeEvent) {
        let expect = RuntimeEvent::Output(RuntimeOutput::ToServer(expect.as_ref().to_owned()));
        assert_eq!(expect, actual);
    }

    fn new_runtime() -> Result<(Runtime, Receiver<Event>)> {
        let (evttx, evtrx) = unbounded();
        let rt = Runtime::new(evttx, &conf::Config::default());
        rt.init()?;
        Ok((rt, evtrx))
    }
}
