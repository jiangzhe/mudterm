use crate::codec::{Codec, MudCodec};
use crate::conf;
use crate::error::{Error, Result};
use crate::runtime::alias::Alias;
use crate::runtime::alias::Aliases;
use crate::runtime::cache::{CacheText, InlineStyle};
use crate::runtime::init::init_lua;
use crate::runtime::model::ModelStore;
use crate::runtime::queue::{ActionQueue, OutputQueue};
use crate::runtime::trigger::Trigger;
use crate::runtime::trigger::Triggers;
use crate::runtime::vars::Variables;
use crate::runtime::RuntimeOutput;
use crate::ui::ansi::AnsiParser;
use crate::ui::line::{Line, Lines, RawLine};
use crate::ui::UserOutput;
use std::collections::VecDeque;
use std::fs::File;
use std::io::{Read, Write};

// 别名回调存储于Lua脚本引擎的全局变量表中
pub(crate) const GLOBAL_ALIAS_CALLBACKS: &str = "_global_alias_callbacks";
// 触发器回调存储于Lua脚本引擎的全局变量表中
pub(crate) const GLOBAL_TRIGGER_CALLBACKS: &str = "_global_trigger_callbacks";
// 计时器回调存储于Lua脚本引擎的全局变量表中
pub(crate) const GLOBAL_TIMER_CALLBACKS: &str = "_global_timer_callbacks";

/// 运行时操作
#[derive(Debug, Clone, PartialEq)]
pub enum EngineAction {
    SwitchCodec(Codec),
    CreateAlias(Alias),
    DeleteAlias(String),
    EnableAliasGroup(String, bool),
    CreateTrigger(Trigger),
    DeleteTrigger(String),
    EnableTriggerGroup(String, bool),
    LoadFile(String),
    // ExecuteUserCmd(String),
    // ExecuteUserScript(String),
    ExecuteUserOutput(UserOutput),
    ParseWorldBytes(Vec<u8>),
    // 将文本发送到UI界面，原始文本可选（来源于服务端）
    SendLineToUI(Line, Option<RawLine>),
    SendToServer(String),
    ProcessWorldLines(Vec<RawLine>),
}

/// 用于执行各类运行时操作
pub struct Engine {
    lua: mlua::Lua,
    vars: Variables,
    actq: VecDeque<EngineAction>,
    // 临时队列，用于脚本执行生成操作的临时处理队列
    tmpq: ActionQueue,
    mud_codec: MudCodec,
    parser: AnsiParser,
    cache: CacheText,
    aliases: Aliases,
    triggers: Triggers,
    cmd_delim: char,
    send_empty_cmd: bool,
    logger: Option<File>,
}

impl Engine {
    pub fn new(config: &conf::Config) -> Self {
        Self {
            // evttx,
            lua: mlua::Lua::new(),
            vars: Variables::new(),
            actq: VecDeque::new(),
            tmpq: ActionQueue::new(),
            mud_codec: MudCodec::new(),
            parser: AnsiParser::new(),
            // only allow up to 5 lines for trigger
            cache: CacheText::new(5, 10),
            aliases: Aliases::new(),
            triggers: Triggers::new(),
            cmd_delim: config.term.cmd_delim,
            send_empty_cmd: config.term.send_empty_cmd,
            logger: None,
        }
    }

    pub fn set_logger(&mut self, logger: File) {
        self.logger = Some(logger);
    }

    pub fn init(&mut self) -> Result<()> {
        init_lua(&self.lua, &self.vars, &self.tmpq)?;
        Ok(())
    }

    /// 推送操作
    pub fn push(&mut self, action: EngineAction) {
        self.actq.push_back(action);
    }

    /// 执行操作队列
    pub fn apply(&mut self) -> Vec<RuntimeOutput> {
        let mut output = OutputQueue::new();
        self.apply_tmpq(&mut output);
        while let Some(action) = self.actq.pop_front() {
            self.run_action(action, &mut output);
            self.apply_tmpq(&mut output);
        }
        output.into_vec()
    }

    /// 执行单个操作    
    fn run_action(&mut self, action: EngineAction, output: &mut OutputQueue) {
        match action {
            EngineAction::SwitchCodec(code) => {
                self.mud_codec.switch_codec(code);
            }
            EngineAction::CreateAlias(alias) => {
                let name = alias.model.name.to_owned();
                if let Err(alias) = self.create_alias(alias) {
                    let err_lines = Lines::fmt_err(format!("创建别名失败：{:?}", alias));
                    for err_line in err_lines.into_vec() {
                        self.tmpq.push(EngineAction::SendLineToUI(err_line, None));
                    }
                    // 注销回调函数，忽略错误
                    if let Err(e) = self.delete_alias_callback(&name) {
                        log::warn!("delete alias callback error {}", e);
                    }
                }
            }
            EngineAction::DeleteAlias(name) => {
                if let Err(e) = self.delete_alias(&name) {
                    log::warn!("delete alias error {}", e);
                }
            }
            EngineAction::EnableAliasGroup(name, enabled) => {
                if let Err(e) = self.enable_alias_group(&name, enabled) {
                    log::warn!("enable alias group error {}", e);
                }
            }
            EngineAction::CreateTrigger(trigger) => {
                let name = trigger.model.name.to_owned();
                if let Err(trigger) = self.create_trigger(trigger) {
                    let err_lines = Lines::fmt_err(format!("创建触发器失败：{:?}", trigger));
                    for err_line in err_lines.into_vec() {
                        self.tmpq.push(EngineAction::SendLineToUI(err_line, None));
                    }
                    // 注销回调函数，忽略错误
                    if let Err(e) = self.delete_trigger_callback(&name) {
                        log::warn!("delete trigger callback error {}", e);
                    }
                }
            }
            EngineAction::DeleteTrigger(name) => {
                if let Err(e) = self.delete_trigger(&name) {
                    log::warn!("delete trigger error {}", e);
                }
            }
            EngineAction::EnableTriggerGroup(name, enabled) => {
                if let Err(e) = self.enable_trigger_group(&name, enabled) {
                    log::warn!("enable trigger group error {}", e);
                }
            }
            EngineAction::LoadFile(path) => {
                if let Err(e) = self.load_file(&path) {
                    log::warn!("load file error {}", e);
                }
            }
            EngineAction::ExecuteUserOutput(output) => match output {
                UserOutput::Cmd(cmd) => self.process_user_cmd(cmd),
                UserOutput::Script(script) => self.process_user_script(script),
            },
            EngineAction::ParseWorldBytes(bs) => {
                if let Err(e) = self.parse_world_bytes(bs) {
                    log::warn!("parse raw bytes error {}", e);
                }
            }
            EngineAction::ProcessWorldLines(lines) => {
                // 这里可能产生递归调用
                self.process_world_lines(lines, output);
            }
            // 所有IO输出必定经过以下两个操作
            EngineAction::SendLineToUI(line, rawline) => {
                // output.send_styled_line(line);
                if let Some(rawline) = rawline {
                    output.send_line(rawline, line);
                } else {
                    output.send_styled_line(line);
                }
            }
            EngineAction::SendToServer(cmd) => {
                output.send_cmd(cmd, self.mud_codec.encoder());
            }
        }
    }

    /// 执行任意脚本，用户可通过UI界面直接输入脚本
    fn exec_script(&self, input: impl AsRef<str>) -> Result<()> {
        log::debug!("Executing script {}", input.as_ref());
        let input = input.as_ref();
        self.lua.load(input).exec()?;
        Ok(())
    }

    /// 执行别名回调
    fn exec_alias(&self, name: String, text: String) -> Result<()> {
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
        log::debug!("Deleting alias {}", name);
        self.delete_alias_callback(name)?;
        self.aliases.remove(name);
        Ok(())
    }

    /// 启用/禁用别名组
    fn enable_alias_group(&mut self, name: &str, enabled: bool) -> Result<()> {
        log::debug!("Enabling alias group {}, enabled={}", name, enabled);
        let n = self.aliases.enable_group(name, enabled);
        log::trace!("{} aliases effected", n);
        Ok(())
    }

    /// 执行触发器
    fn exec_trigger(
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

    // 启用/禁用触发器组
    fn enable_trigger_group(&mut self, name: &str, enabled: bool) -> Result<()> {
        log::debug!("Enabling trigger group {}, enabled={}", name, enabled);
        let n = self.triggers.enable_group(name, enabled);
        log::trace!("{} triggers effected", n);
        Ok(())
    }

    // 加载外部文件
    fn load_file(&mut self, path: &str) -> Result<()> {
        log::debug!("Loading file {}", path);
        let mut file = File::open(path)?;
        let mut text = String::new();
        file.read_to_string(&mut text)?;
        self.lua.load(&text).exec()?;
        Ok(())
    }

    /// 这是对原始字节流的处理，这里仅解码并处理换行
    fn parse_world_bytes(&mut self, bs: Vec<u8>) -> Result<()> {
        let s = self.mud_codec.decode(&bs);
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
        log::trace!("coded {} lines from mud", lines.len());
        self.tmpq.push(EngineAction::ProcessWorldLines(lines));
        Ok(())
    }

    // 处理世界文本
    fn process_world_line(&mut self, raw: RawLine) {
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
        self.tmpq
            .push(EngineAction::SendLineToUI(styled, Some(raw)));
        // 使用is_match预先匹配，最多一个触发器
        if let Some((trigger, text, styles)) = self.triggers.trigger_first(&self.cache) {
            if let Err(e) = self.exec_trigger(trigger, text, styles) {
                let err_lines = Lines::fmt_err(e.to_string());
                for err_line in err_lines.into_vec() {
                    self.tmpq.push(EngineAction::SendLineToUI(err_line, None));
                }
            }
            // 对OneShot触发器进行删除
            if trigger.model.extra.one_shot() {
                self.tmpq
                    .push(EngineAction::DeleteTrigger(trigger.model.name.to_owned()));
            }
        }
    }

    // 处理多行世界文本
    // 由于每一行都肯能触发脚本，改变后续文本的处理方式，因此需要在处理完
    // 每一行以后，运行临时操作队列直到其清空，方可处理下一行
    fn process_world_lines(
        &mut self,
        lines: impl IntoIterator<Item = RawLine>,
        output: &mut OutputQueue,
    ) {
        for line in lines {
            self.process_world_line(line);
            // 这里，每处理一行，都需要将操作立即执行
            // 否则可能导致先前行开启/关闭的触发器对后续行
            // 的不正确的影响。
            self.apply_tmpq(output);
        }
    }

    // 执行临时队列，直到队列为空
    fn apply_tmpq(&mut self, output: &mut OutputQueue) {
        const ITER_CNT: usize = 30;
        let mut i = 0;
        loop {
            if self.tmpq.is_empty() {
                log::trace!("apply tmp action queue with iteration={}", i);
                return;
            }
            if i >= ITER_CNT {
                log::error!("reach iteration limit {} on tmp action queue processing", i);
                log::warn!("tmpq.len={}", self.tmpq.len());
                log::warn!("tmpq={:?}", self.tmpq);
                return;
            }
            let actions = self.tmpq.drain_all();
            for action in actions {
                self.run_action(action, output);
            }
            i += 1;
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
            self.tmpq.push(EngineAction::SendToServer("\n".to_owned()));
            return;
        }
        for cmd in cmds {
            match cmd {
                PostCmd::Raw(mut s) => {
                    if !s.ends_with('\n') {
                        s.push('\n');
                    }
                    // self.outq.send_cmd(s);
                    self.tmpq.push(EngineAction::SendToServer(s));
                }
                PostCmd::Alias { name, text } => {
                    if let Err(e) = self.exec_alias(name, text) {
                        let err_lines = Lines::fmt_err(e.to_string());
                        for err_line in err_lines.into_vec() {
                            self.tmpq.push(EngineAction::SendLineToUI(err_line, None));
                        }
                    }
                }
            }
        }
    }

    /// 处理用户脚本
    fn process_user_script(&mut self, script: String) {
        if let Err(e) = self.exec_script(&script) {
            let err_lines = Lines::fmt_err(e.to_string());
            for err_line in err_lines.into_vec() {
                self.tmpq.push(EngineAction::SendLineToUI(err_line, None));
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
    use crate::ui::line::{Line, RawLine, RawLines};
    use crate::ui::span::Span;
    use crate::ui::style::Style;
    use crate::ui::UserOutput;

    #[test]
    fn test_engine_single_user_cmd() {
        let mut engine = new_engine().unwrap();
        engine.push(EngineAction::ExecuteUserOutput(UserOutput::Cmd(
            "hp".to_owned(),
        )));
        let mut evts = engine.apply();
        assert_eq!(1, evts.len());
        assert_eq!(
            RuntimeOutput::ToServer(b"hp\n".to_vec()),
            evts.pop().unwrap()
        );
    }

    #[test]
    fn test_engine_multi_user_cmds() {
        let mut engine = new_engine().unwrap();
        engine.push(EngineAction::ExecuteUserOutput(UserOutput::Cmd(
            "hp;say hi".to_owned(),
        )));
        let mut evts = engine.apply();
        assert_eq!(1, evts.len());
        assert_eq!(
            RuntimeOutput::ToServer(b"hp\nsay hi\n".to_vec()),
            evts.pop().unwrap()
        );

        engine.push(EngineAction::ExecuteUserOutput(UserOutput::Cmd(
            "hp\nsay hi".to_owned(),
        )));
        let mut evts = engine.apply();
        assert_eq!(1, evts.len());
        assert_eq!(
            RuntimeOutput::ToServer(b"hp\nsay hi\n".to_vec()),
            evts.pop().unwrap()
        );
    }

    #[test]
    fn test_engine_simple_alias() {
        let mut engine = new_engine().unwrap();
        engine
            .lua
            .load(
                r#"
        local n = function() Send("north") end
        CreateAlias("alias-n", "map", "^n$", alias_flag.Enabled, n)
        "#,
            )
            .exec()
            .unwrap();
        engine.apply();
        // start user output
        engine.push(EngineAction::ExecuteUserOutput(UserOutput::Cmd(
            "n".to_owned(),
        )));
        let mut outputs = engine.apply();
        assert_eq!(1, outputs.len());
        assert_eq!(
            RuntimeOutput::ToServer(b"north\n".to_vec()),
            outputs.pop().unwrap()
        );
    }

    #[test]
    fn test_engine_complex_alias() {
        let mut engine = new_engine().unwrap();
        engine
            .lua
            .load(
                r#"
        local m = function(name, line, wildcards) Send(wildcards[1]..wildcards[2]) end
        CreateAlias("alias-m", "number", "^num (\\d+)\\s+(\\d+)$", alias_flag.Enabled, m) 
        "#,
            )
            .exec()
            .unwrap();
        engine.apply();
        engine.push(EngineAction::ExecuteUserOutput(UserOutput::Cmd(
            "x;num 123 456".to_owned(),
        )));
        let mut outputs = engine.apply();
        assert_eq!(1, outputs.len());
        assert_eq!(
            RuntimeOutput::ToServer(b"x\n123456\n".to_vec()),
            outputs.pop().unwrap()
        );
    }

    #[test]
    fn test_engine_alias_add_trigger() {
        let mut engine = new_engine().unwrap();
        engine
            .lua
            .load(
                r#"
            local addtr = function()
                local trcb = function() end
                CreateTrigger("tr1", "trg", "^say hi$", trigger_flag.Enabled, 1, trcb)
            end
            CreateAlias("alias-tr", "addtr", "^addtr$", alias_flag.Enabled, addtr)
            "#,
            )
            .exec()
            .unwrap();
        engine.apply();
        engine.push(EngineAction::ExecuteUserOutput(UserOutput::Cmd(
            "addtr".to_owned(),
        )));
        let outputs = engine.apply();
        assert!(outputs.is_empty());
        assert_eq!(1, engine.triggers.len());
    }

    #[test]
    fn test_engine_simple_trigger() {
        let mut engine = new_engine().unwrap();
        engine.push(EngineAction::SwitchCodec(Codec::Utf8));
        engine
            .lua
            .load(
                r#"
            local f = function() Send("triggered") end
            CreateTrigger("trigger-f", "trg", "^张三走了过来。$", trigger_flag.Enabled, 1, f)
            "#,
            )
            .exec()
            .unwrap();
        engine.apply();
        engine.push(EngineAction::ProcessWorldLines(vec![RawLine::new(
            "张三走了过来。\r\n",
        )]));
        let mut evts = engine.apply();
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
            RuntimeOutput::ToServer(b"triggered\n".to_vec()),
            evts.remove(0)
        );
    }

    #[test]
    fn test_engine_multiline_trigger() {
        let mut engine = new_engine().unwrap();
        engine.push(EngineAction::SwitchCodec(Codec::Utf8));
        engine.lua.load(
            r#"
            local m = function() Send("triggered") end
            CreateTrigger("trigger-m", "trg", "^张三走了过来。\r\n李四走了过来。$", trigger_flag.Enabled, 2, m)
            "#
        ).exec().unwrap();
        engine.apply();
        engine.push(EngineAction::ProcessWorldLines(vec![
            RawLine::new("张三走了过来。\r\n"),
            RawLine::new("李四走了过来。\r\n"),
        ]));
        let mut evts = engine.apply();
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
            RuntimeOutput::ToServer(b"triggered\n".to_vec()),
            evts.remove(0)
        );
    }

    #[test]
    fn test_engine_wildcard_trigger() {
        let mut engine = new_engine().unwrap();
        // 默认使用utf8编码
        engine.push(EngineAction::SwitchCodec(Codec::Utf8));
        engine
            .lua
            .load(
                r#"
            local f = function(name, line, wildcards) Send(wildcards[1]) end
            CreateTrigger("trigger-f", "trg", "^(.*)走了过来。$", trigger_flag.Enabled, 1, f)
            "#,
            )
            .exec()
            .unwrap();
        engine.apply();
        engine.push(EngineAction::ProcessWorldLines(vec![RawLine::new(
            "张三走了过来。\r\n",
        )]));
        let mut evts = engine.apply();
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
            RuntimeOutput::ToServer("张三\n".as_bytes().to_vec()),
            evts.remove(0)
        );
    }

    #[test]
    fn test_engine_oneshot_trigger() {
        let mut engine = new_engine().unwrap();
        // 默认使用utf8编码
        engine.push(EngineAction::SwitchCodec(Codec::Utf8));
        engine.lua.load(r#"
            local oneshot = function(name, line, wildcards) Send(wildcards[1]) end
            CreateTrigger("trigger-oneshot", "trg", "^(.*)走了过来。$", trigger_flag.Enabled + trigger_flag.OneShot, 1, oneshot)
        "#).exec().unwrap();
        engine.apply();
        engine.push(EngineAction::ProcessWorldLines(vec![RawLine::new(
            "张三走了过来。\r\n",
        )]));
        let mut evts = engine.apply();
        let mut rawlines = RawLines::unbounded();
        rawlines.push_line(RawLine::new("张三走了过来。\r\n"));
        let mut lines = Lines::new();
        lines.push_line(Line::new(vec![Span::new(
            "张三走了过来。\r\n",
            Style::default(),
        )]));
        assert_eq!(RuntimeOutput::ToUI(rawlines, lines), evts.remove(0));
        assert_eq!(
            RuntimeOutput::ToServer("张三\n".as_bytes().to_vec()),
            evts.remove(0)
        );
        assert_eq!(0, engine.triggers.len());
    }

    fn new_engine() -> Result<Engine> {
        let mut engine = Engine::new(&crate::conf::Config::default());
        engine.init()?;
        Ok(engine)
    }
}
