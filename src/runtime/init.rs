use crate::codec::Codec;
use crate::error::{Error, Result};
use crate::runtime::alias::{AliasFlags, AliasModel};
use crate::runtime::engine;
use crate::runtime::engine::EngineAction;
use crate::runtime::queue::ActionQueue;
use crate::runtime::trigger::{TriggerExtra, TriggerFlags, TriggerModel};
use crate::runtime::vars::Variables;
use crate::ui::line::Line;
use crate::ui::style::{Color, Style};
use crate::ui::UserOutput;
use mlua::Lua;
use uuid::Uuid;

/// 初始化运行时
///
/// 1. 定义全局变量表，Lua脚本通过SetVariable()和GetVariable()函数
///    对其中的值进行设置和查询
/// 2. 定义Lua脚本引擎中的的核心函数
///    有一部分函数借鉴了MUSHClient的函数签名。
pub fn init_lua(lua: &Lua, vtl: &Variables, tmpq: &ActionQueue) -> Result<()> {
    let globals = lua.globals();

    // 初始化SetVariable函数
    let vars = vtl.clone();
    let set_variable = lua.create_function(move |_, (k, v): (String, String)| {
        log::trace!("SetVariable function called");
        vars.insert(k, v);
        Ok(())
    })?;
    globals.set("SetVariable", set_variable)?;

    // 初始化GetVariable函数
    let vars = vtl.clone();
    let get_variable = lua.create_function(move |_, k: String| {
        log::trace!("GetVariable function called");
        match vars.get(&k) {
            Some(v) => Ok(Some(v.to_owned())),
            None => Ok(None),
        }
    })?;
    globals.set("GetVariable", get_variable)?;

    // 初始化SwitchCodec函数
    let queue = tmpq.clone();
    let switch_codec = lua.create_function(move |_, code: String| {
        log::trace!("SwitchCodec function called");
        let new_code = match &code.to_lowercase()[..] {
            "gbk" => Codec::Gb18030,
            "utf8" | "utf-8" => Codec::Utf8,
            "big5" => Codec::Big5,
            _ => return Ok(()),
        };
        queue.push(EngineAction::SwitchCodec(new_code));
        Ok(())
    })?;
    globals.set("SwitchCodec", switch_codec)?;

    // 重写print函数
    let print = lua.create_function(move |_, _: ()| Ok(()))?;
    globals.set("print", print)?;

    // 初始化Send函数
    let queue = tmpq.clone();
    let send = lua.create_function(move |_, s: String| {
        log::trace!("Send function called");
        queue.push(EngineAction::ExecuteUserOutput(UserOutput::Cmd(s)));
        Ok(())
    })?;
    globals.set("Send", send)?;

    // 初始化Note函数
    let queue = tmpq.clone();
    let note = lua.create_function(move |_, s: String| {
        log::trace!("Note function called");
        // queue.send_styled_line(Line::fmt_note(s));
        queue.push(EngineAction::SendLineToUI(Line::fmt_note(s), None));
        Ok(())
    })?;
    globals.set("Note", note)?;

    // 初始化ColourNote函数
    let queue = tmpq.clone();
    let colour_note = lua.create_function(move |_, (fg, bg, text): (String, String, String)| {
        log::trace!("ColourNote function called");
        let style = Style::default()
            .fg(Color::from_str_or_default(fg, Color::Reset))
            .bg(Color::from_str_or_default(bg, Color::Reset));
        queue.push(EngineAction::SendLineToUI(
            Line::fmt_with_style(text, style),
            None,
        ));
        Ok(())
    })?;
    globals.set("ColourNote", colour_note)?;

    // 初始化GetUniqueID函数
    let get_unique_id = lua.create_function(move |_, _: ()| {
        let id = Uuid::new_v4();
        Ok(id.to_simple().to_string())
    })?;
    globals.set("GetUniqueID", get_unique_id)?;

    // 别名常量
    let alias_flag: mlua::Table = lua.create_table()?;
    alias_flag.set("Enabled", 1)?;
    alias_flag.set("KeepEvaluating", 8)?;
    globals.set("alias_flag", alias_flag)?;

    // 别名回调注册表
    let alias_callbacks = lua.create_table()?;
    lua.globals()
        .set(engine::GLOBAL_ALIAS_CALLBACKS, alias_callbacks)?;

    // 初始化CreateAlias函数
    let queue = tmpq.clone();
    let create_alias = lua.create_function(
        move |lua,
              (name, group, pattern, flags, func): (
            String,
            String,
            String,
            u16,
            mlua::Function,
        )| {
            log::trace!("CreateAlias function called");
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

            let alias_callbacks: mlua::Table = lua.globals().get(engine::GLOBAL_ALIAS_CALLBACKS)?;
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
            queue.push(EngineAction::CreateAlias(alias));
            Ok(())
        },
    )?;
    globals.set("CreateAlias", create_alias)?;

    // 初始化DeleteAlias函数
    let queue = tmpq.clone();
    let delete_alias = lua.create_function(move |_, name: String| {
        log::trace!("DeleteAlias function called");
        queue.push(EngineAction::DeleteAlias(name));
        Ok(())
    })?;
    globals.set("DeleteAlias", delete_alias)?;

    // 触发器常量
    let trigger_flag: mlua::Table = lua.create_table()?;
    trigger_flag.set("Enabled", 1)?;
    trigger_flag.set("KeepEvaluating", 8)?;
    trigger_flag.set("OneShot", 32768)?;
    globals.set("trigger_flag", trigger_flag)?;

    // 触发器回调注册表
    let trigger_callbacks = lua.create_table()?;
    lua.globals()
        .set(engine::GLOBAL_TRIGGER_CALLBACKS, trigger_callbacks)?;

    // 初始化CreateTrigger函数
    let queue = tmpq.clone();
    let create_trigger = lua.create_function(
        move |lua,
              (name, group, pattern, flags, match_lines, func): (
            String,
            String,
            String,
            u16,
            u8,
            mlua::Function,
        )| {
            log::trace!("CreateTrigger function called");
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

            let trigger_callbacks: mlua::Table =
                lua.globals().get(engine::GLOBAL_TRIGGER_CALLBACKS)?;
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
            queue.push(EngineAction::CreateTrigger(trigger));
            Ok(())
        },
    )?;
    globals.set("CreateTrigger", create_trigger)?;

    // 初始化DeleteTrigger函数
    let queue = tmpq.clone();
    let delete_trigger = lua.create_function(move |_, name: String| {
        log::trace!("DeleteTrigger function called");
        queue.push(EngineAction::DeleteTrigger(name));
        Ok(())
    })?;
    globals.set("DeleteTrigger", delete_trigger)?;

    // 初始化EnableTriggerGroup函数
    let queue = tmpq.clone();
    let enable_trigger_group = lua.create_function(move |_, (name, enabled): (String, bool)| {
        log::trace!("EnableTriggerGroup function called");
        queue.push(EngineAction::EnableTriggerGroup(name, enabled));
        Ok(())
    })?;
    globals.set("EnableTriggerGroup", enable_trigger_group)?;

    // 初始化LoadFile函数
    let queue = tmpq.clone();
    let load_file = lua.create_function(move |_, path: String| {
        queue.push(EngineAction::LoadFile(path));
        Ok(())
    })?;
    globals.set("LoadFile", load_file)?;

    Ok(())
}
