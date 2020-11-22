pub mod alias;
pub mod cache;
pub mod model;
pub mod queue;
pub mod sub;
pub mod timer;
pub mod trigger;

use crate::codec::{Codec, MudCodec};
use crate::conf;
use crate::error::{Error, Result};
use crate::event::{Event, NextStep};
use crate::ui::ansi::AnsiParser;
use crate::ui::line::{Line, Lines, RawLine, RawLines};
use crate::ui::style::{Color, Style};
use crate::ui::UserOutput;
use alias::{Alias, AliasFlags, AliasModel, Aliases};
use cache::{CacheText, InlineStyle};
use crossbeam_channel::Sender;
use mlua::Lua;
use model::ModelStore;
use queue::OutputQueue;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::fs::File;
use std::hash::Hash;
use std::io::{Read, Write};
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use sub::{Sub, SubParser};
use trigger::{Trigger, TriggerExtra, TriggerFlags, TriggerModel, Triggers};
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
    /// 发送给UI的文本（包含原始文本，以及格式解析后的文本）
    ToUI(RawLines, Lines),
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
    CreateTrigger(Trigger),
    DeleteTrigger(String),
    LoadFile(String),
    ExecuteUserCmd(String),
    ExecuteUserScript(String),
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
    // ANSI解析器
    parser: AnsiParser,
    cache: CacheText,
    aliases: Aliases,
    triggers: Triggers,
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
            cmd_delim: config.term.cmd_delim,
            send_empty_cmd: config.term.send_empty_cmd,
            logger: None,
            parser: AnsiParser::new(),
            // only allow up to 5 lines for trigger
            cache: CacheText::new(5, 10),
            aliases: Aliases::new(),
            triggers: Triggers::new(),
        }
    }

    /// 设置日志输出
    pub fn set_logger(&mut self, logger: File) {
        self.logger = Some(logger);
    }

    /// 向UI线程推送文字
    fn push_line_to_ui(&self, line: Line) {
        // 直接向队列中推送，跳过触发器阶段
        self.queue.send_styled_line(line);
    }

    /// 将错误信息推送给UI
    pub fn push_err_to_ui(&self, err: impl Into<String>) {
        let mut lines: String = err.into();
        if lines.ends_with('\n') {
            lines.truncate(lines.len() - 1);
        }
        for line in lines.split('\n') {
            let line = if line.ends_with('\r') {
                &line[..line.len() - 1]
            } else {
                line
            };
            let line = Line::fmt_err(line);
            self.push_line_to_ui(line);
        }
    }

    /// 将UTF-8字符串编码为MUD服务器编码（如GBK，Big5或仍然为UTF-8）
    pub fn encode(&mut self, s: impl AsRef<str>) -> Result<Vec<u8>> {
        self.mud_codec.encode(s.as_ref())
    }

    /// 执行任意脚本，用户可通过UI界面直接输入脚本
    pub fn exec_script(&self, input: impl AsRef<str>) -> Result<()> {
        log::debug!("Executing script {}", input.as_ref());
        let input = input.as_ref();
        self.lua.load(input).exec()?;
        Ok(())
    }

    /// 执行别名回调
    pub fn exec_alias(&self, name: String, text: String) -> Result<()> {
        log::debug!("Executing alias {}", name);
        log::trace!("matched text={}", text);
        let alias = self.aliases.get(&name).ok_or_else(|| {
            Error::RuntimeError(format!("alias '{}' not found with text '{}'", &name, &text))
        })?;
        let wildcards = alias.captures(&text)?;
        let callbacks: mlua::Table = self.lua.globals().get(GLOBAL_ALIAS_CALLBACKS)?;
        let func: mlua::Function = callbacks.get(&alias.model.name[..])?;
        func.call((name, text, wildcards))?;
        Ok(())
    }

    /// 创建别名
    fn create_alias(&mut self, alias: Alias) -> std::result::Result<(), Alias> {
        log::debug!("Creating alias {}", alias.model.name);
        log::trace!("pattern={}", alias.model.pattern);
        self.aliases.add(alias)
    }

    /// 删除别名回调：回调注册在Lua全局回调表中
    fn delete_alias_callback(&mut self, name: &str) -> Result<()> {
        let alias_callbacks: mlua::Table = self.lua.globals().get(GLOBAL_ALIAS_CALLBACKS)?;
        alias_callbacks.set(name, mlua::Value::Nil)?;
        Ok(())
    }

    /// 删除别名
    fn delete_alias(&mut self, name: &str) -> Result<()> {
        log::info!("Deleting alias {}", name);
        self.delete_alias_callback(name)?;
        self.aliases.remove(name);
        Ok(())
    }

    /// 执行触发器
    pub fn exec_trigger(
        &self,
        trigger: &Trigger,
        text: String,
        styles: Vec<InlineStyle>,
    ) -> Result<()> {
        log::debug!("Executing trigger {}", trigger.model.name);
        log::trace!("matched text={}", text);
        let callbacks: mlua::Table = self.lua.globals().get(GLOBAL_TRIGGER_CALLBACKS)?;
        let func: mlua::Function = callbacks.get(&trigger.model.name[..])?;
        let wildcards = trigger.captures(&text)?;
        func.call((trigger.model.name.to_owned(), text, wildcards, styles))?;
        Ok(())
    }

    /// 处理用户输出：命名参考MUSHClient
    pub fn process_user_output(&mut self, output: UserOutput) {
        match output {
            UserOutput::Script(script) => {
                self.queue.push(RuntimeEvent::ImmediateAction(
                    RuntimeAction::ExecuteUserScript(script),
                ));
            }
            UserOutput::Cmd(cmd) => {
                self.queue.push(RuntimeEvent::ImmediateAction(
                    RuntimeAction::ExecuteUserCmd(cmd),
                ));
            }
        }
    }

    /// 创建触发器
    fn create_trigger(&mut self, trigger: Trigger) -> std::result::Result<(), Trigger> {
        log::debug!("Creating trigger {}", trigger.model.name);
        log::trace!("pattern={}", trigger.model.pattern);
        self.triggers.add(trigger)
    }

    /// 删除触发器回调
    fn delete_trigger_callback(&mut self, name: &str) -> Result<()> {
        let trigger_callbacks: mlua::Table = self.lua.globals().get(GLOBAL_TRIGGER_CALLBACKS)?;
        trigger_callbacks.set(name, mlua::Value::Nil)?;
        Ok(())
    }

    /// 删除触发器
    fn delete_trigger(&mut self, name: &str) -> Result<()> {
        log::debug!("Deleting trigger {}", name);
        self.delete_trigger_callback(name)?;
        self.triggers.remove(name);
        Ok(())
    }

    // 加载外部文件
    fn load_file(&mut self, path: &str) -> Result<()> {
        let mut file = File::open(path)?;
        let mut text = String::new();
        file.read_to_string(&mut text)?;
        self.lua.load(&text).exec()?;
        Ok(())
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
            self.queue.send_cmd("\n".to_owned());
            return;
        }
        for cmd in cmds {
            match cmd {
                PostCmd::Raw(mut s) => {
                    if !s.ends_with('\n') {
                        s.push('\n');
                    }
                    self.queue.send_cmd(s);
                }
                PostCmd::Alias { name, text } => {
                    if let Err(e) = self.exec_alias(name, text) {
                        self.push_err_to_ui(e.to_string());
                    }
                }
            }
        }
    }

    /// 处理用户脚本
    fn process_user_script(&mut self, script: String) {
        if let Err(e) = self.exec_script(&script) {
            self.push_err_to_ui(e.to_string());
        }
    }

    pub fn process_world_line(&mut self, raw: RawLine) {
        // todo: 需要提前处理文字格式，以便于触发器使用
        self.parser.fill(raw.as_ref());
        let mut styled = vec![];
        while let Some(span) = self.parser.next_span() {
            styled.push(span);
        }
        let styled = Line::new(styled);
        // 添加进文本缓存，供触发器进行匹配
        self.cache.push_line(&styled);
        // 推送到事件队列
        self.queue.push_line(raw, styled);
        // 使用is_match预先匹配，最多一个触发器
        if let Some((trigger, text, styles)) = self.triggers.trigger_first(&self.cache) {
            if let Err(e) = self.exec_trigger(trigger, text, styles) {
                self.push_err_to_ui(e.to_string());
            }
            // 对OneShot触发器进行删除
            if trigger.model.extra.one_shot() {
                self.queue
                    .push(RuntimeEvent::ImmediateAction(RuntimeAction::DeleteTrigger(
                        trigger.model.name.to_owned(),
                    )));
            }
        }
    }

    pub fn process_world_lines(&mut self, lines: impl IntoIterator<Item = RawLine>) {
        for line in lines {
            self.process_world_line(line);
        }
    }

    /// 这是对原始字节流的处理，这里仅解码并处理换行
    pub fn process_bytes_from_mud(&mut self, bs: &[u8]) -> Result<()> {
        let s = self.mud_codec.decode(bs);
        // let s: Arc<str> = Arc::from(s);
        // log server output with ansi sequence
        if let Some(logger) = self.logger.as_mut() {
            logger.write_all(s.as_bytes())?;
        }

        // here just split into lines
        let mut lines = Vec::new();
        let mut start = 0usize;
        while let Some(end) = s[start..].find('\n') {
            let end = start + end;
            let line = RawLine::new(s[start..end + 1].to_owned());
            lines.push(line);
            start = end + 1;
        }
        if start < s.len() {
            let line = RawLine::new(s[start..].to_owned());
            lines.push(line);
        }
        // send to global event bus
        log::trace!("coded {} lines from mud", lines.len());
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
                    self.push_line_to_ui(Line::fmt_err(format!("创建别名失败：{:?}", alias)));
                    // 注销回调函数，忽略错误
                    if let Err(e) = self.delete_alias_callback(&name) {
                        log::warn!("delete alias callback error {}", e);
                    }
                }
            }
            RuntimeAction::DeleteAlias(name) => {
                log::trace!("deleting alias '{}'", name);
                if let Err(e) = self.delete_alias(&name) {
                    log::warn!("delete alias error {}", e);
                }
            }
            RuntimeAction::CreateTrigger(trigger) => {
                let name = trigger.model.name.to_owned();
                if let Err(trigger) = self.create_trigger(trigger) {
                    self.push_line_to_ui(Line::fmt_err(format!("创建触发器失败：{:?}", trigger)));
                    // 注销回调函数，忽略错误
                    if let Err(e) = self.delete_trigger_callback(&name) {
                        log::warn!("delete trigger callback error {}", e);
                    }
                }
            }
            RuntimeAction::DeleteTrigger(name) => {
                log::trace!("deleting trigger '{}'", name);
                if let Err(e) = self.delete_trigger(&name) {
                    log::warn!("delete trigger error {}", e);
                }
            }
            RuntimeAction::LoadFile(path) => {
                log::trace!("loading file '{}'", &path);
                if let Err(e) = self.load_file(&path) {
                    log::warn!("load file error {}", e);
                }
            }
            RuntimeAction::ExecuteUserCmd(cmd) => {
                self.process_user_cmd(cmd);
            }
            RuntimeAction::ExecuteUserScript(script) => {
                self.process_user_script(script);
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
        let print = self.lua.create_function(move |_, _: ()| Ok(()))?;
        globals.set("print", print)?;

        // 初始化Send函数
        let queue = self.queue.clone();
        let send = self.lua.create_function(move |_, s: String| {
            log::trace!("Send function called");
            queue.push(RuntimeEvent::ImmediateAction(
                RuntimeAction::ExecuteUserCmd(s),
            ));
            Ok(())
        })?;
        globals.set("Send", send)?;

        // 初始化Note函数
        let queue = self.queue.clone();
        let note = self.lua.create_function(move |_, s: String| {
            log::trace!("Note function called");
            queue.send_styled_line(Line::fmt_note(s));
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
                    queue.send_styled_line(Line::fmt_with_style(text, style));
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
        let delete_alias = self.lua.create_function(move |_, name: String| {
            queue.push(RuntimeEvent::ImmediateAction(RuntimeAction::DeleteAlias(
                name,
            )));
            Ok(())
        })?;
        globals.set("DeleteAlias", delete_alias)?;

        // 触发器常量
        let trigger_flag: mlua::Table = self.lua.create_table()?;
        trigger_flag.set("Enabled", 1)?;
        trigger_flag.set("KeepEvaluating", 8)?;
        trigger_flag.set("OneShot", 32768)?;
        globals.set("trigger_flag", trigger_flag)?;

        // 触发器回调注册表
        let trigger_callbacks = self.lua.create_table()?;
        self.lua
            .globals()
            .set(GLOBAL_TRIGGER_CALLBACKS, trigger_callbacks)?;

        // 初始化CreateTrigger函数
        let queue = self.queue.clone();
        let create_trigger = self.lua.create_function(
            move |lua,
                  (name, group, pattern, flags, match_lines, func): (
                String,
                String,
                String,
                u16,
                u8,
                mlua::Function,
            )| {
                if pattern.is_empty() {
                    return Err(mlua::Error::external(Error::RuntimeError(
                        "empty pattern not allowed when creating trigger".to_owned(),
                    )));
                }
                let flags = TriggerFlags::from_bits(flags).ok_or_else(|| {
                    mlua::Error::external(Error::RuntimeError(format!(
                        "invalid trigger flags {}",
                        flags
                    )))
                })?;

                let trigger_callbacks: mlua::Table = lua.globals().get(GLOBAL_TRIGGER_CALLBACKS)?;
                if trigger_callbacks.contains_key(name.to_owned())? {
                    return Err(mlua::Error::external(Error::RuntimeError(format!(
                        "trigger callback '{}' already exists",
                        &name
                    ))));
                }
                let model = TriggerModel {
                    name,
                    group,
                    pattern,
                    extra: TriggerExtra { match_lines, flags },
                };
                let trigger = model.compile().map_err(|e| mlua::Error::external(e))?;
                // 同alias
                trigger_callbacks.set(trigger.model.name.to_owned(), func)?;
                queue.push(RuntimeEvent::ImmediateAction(RuntimeAction::CreateTrigger(
                    trigger,
                )));
                Ok(())
            },
        )?;
        globals.set("CreateTrigger", create_trigger)?;

        let queue = self.queue.clone();
        let delete_trigger = self.lua.create_function(move |_, name: String| {
            queue.push(RuntimeEvent::ImmediateAction(RuntimeAction::DeleteTrigger(
                name,
            )));
            Ok(())
        })?;
        globals.set("DeleteTrigger", delete_trigger)?;

        // 初始化LoadFile函数
        let queue = self.queue.clone();
        let load_file = self.lua.create_function(move |_, path: String| {
            queue.push(RuntimeEvent::ImmediateAction(RuntimeAction::LoadFile(path)));
            Ok(())
        })?;
        globals.set("LoadFile", load_file)?;
        Ok(())
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

/// 预处理后的命令，用户原始命令，或经过别名匹配后的脚本名
#[derive(Debug, Clone)]
pub enum PostCmd {
    Raw(String),
    Alias { name: String, text: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::span::Span;
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
        rt.lua
            .load(
                r#"
        local n = function() Send("north") end
        CreateAlias("alias-n", "map", "^n$", alias_flag.Enabled, n)
        "#,
            )
            .exec()
            .unwrap();
        rt.process_queue();
        // start user output
        rt.process_user_output(UserOutput::Cmd("n".to_owned()));
        let mut outputs = rt.process_queue();
        assert_eq!(1, outputs.len());
        assert_eq!(
            RuntimeOutput::ToServer("north\n".to_owned()),
            outputs.pop().unwrap()
        );
    }

    #[test]
    fn test_process_complex_alias() {
        let (mut rt, _) = new_runtime().unwrap();
        rt.lua
            .load(
                r#"
        local m = function(name, line, wildcards) Send(wildcards[1]..wildcards[2]) end
        CreateAlias("alias-m", "number", "^num (\\d+)\\s+(\\d+)$", alias_flag.Enabled, m) 
        "#,
            )
            .exec()
            .unwrap();
        rt.process_queue();
        rt.process_user_output(UserOutput::Cmd("x;num 123 456".to_owned()));
        let mut outputs = rt.process_queue();
        assert_eq!(1, outputs.len());
        assert_eq!(
            RuntimeOutput::ToServer("x\n123456\n".to_owned()),
            outputs.pop().unwrap()
        );
    }

    #[test]
    fn test_process_simple_trigger() {
        let (mut rt, _) = new_runtime().unwrap();
        rt.lua
            .load(
                r#"
            local f = function() Send("triggered") end
            CreateTrigger("trigger-f", "trg", "^张三走了过来。$", trigger_flag.Enabled, 1, f)
            "#,
            )
            .exec()
            .unwrap();
        rt.process_queue();
        rt.process_world_line(RawLine::new("张三走了过来。\r\n"));
        let mut evts = rt.process_queue();
        assert_eq!(2, evts.len());
        let mut rawlines = RawLines::unbounded();
        rawlines.push_line(RawLine::new("张三走了过来。\r\n"));
        let mut lines = Lines::new();
        lines.push_line(Line::new(vec![Span::new(
            "张三走了过来。\r\n",
            Style::default(),
        )]));
        assert_eq!(RuntimeOutput::ToUI(rawlines, lines), evts.remove(0));
        assert_eq!(
            RuntimeOutput::ToServer("triggered\n".to_owned()),
            evts.remove(0)
        );
    }

    #[test]
    fn test_process_multiline_trigger() {
        let (mut rt, _) = new_runtime().unwrap();
        rt.lua.load(
            r#"
            local m = function() Send("triggered") end
            CreateTrigger("trigger-m", "trg", "^张三走了过来。\r\n李四走了过来。$", trigger_flag.Enabled, 2, m)
            "#
        ).exec().unwrap();
        rt.process_queue();
        rt.process_world_lines(vec![
            RawLine::new("张三走了过来。\r\n"),
            RawLine::new("李四走了过来。\r\n"),
        ]);
        let mut evts = rt.process_queue();
        let mut rawlines = RawLines::unbounded();
        rawlines.push_line(RawLine::new("张三走了过来。\r\n"));
        rawlines.push_line(RawLine::new("李四走了过来。\r\n"));
        let mut lines = Lines::new();
        lines.push_line(Line::new(vec![Span::new(
            "张三走了过来。\r\n",
            Style::default(),
        )]));
        lines.push_line(Line::new(vec![Span::new(
            "李四走了过来。\r\n",
            Style::default(),
        )]));
        assert_eq!(RuntimeOutput::ToUI(rawlines, lines), evts.remove(0));
        assert_eq!(
            RuntimeOutput::ToServer("triggered\n".to_owned()),
            evts.remove(0)
        );
    }

    #[test]
    fn test_process_wildcard_trigger() {
        let (mut rt, _) = new_runtime().unwrap();
        rt.lua
            .load(
                r#"
            local f = function(name, line, wildcards) Send(wildcards[1]) end
            CreateTrigger("trigger-f", "trg", "^(.*)走了过来。$", trigger_flag.Enabled, 1, f)
            "#,
            )
            .exec()
            .unwrap();
        rt.process_queue();
        rt.process_world_line(RawLine::new("张三走了过来。\r\n"));
        let mut evts = rt.process_queue();
        assert_eq!(2, evts.len());
        let mut rawlines = RawLines::unbounded();
        rawlines.push_line(RawLine::new("张三走了过来。\r\n"));
        let mut lines = Lines::new();
        lines.push_line(Line::new(vec![Span::new(
            "张三走了过来。\r\n",
            Style::default(),
        )]));
        assert_eq!(RuntimeOutput::ToUI(rawlines, lines), evts.remove(0));
        assert_eq!(RuntimeOutput::ToServer("张三\n".to_owned()), evts.remove(0));
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
