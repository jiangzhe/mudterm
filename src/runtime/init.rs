use crate::codec::Codec;
use crate::error::{Error, Result};
use crate::runtime::alias::{AliasFlags, Alias};
use crate::runtime::engine;
use crate::runtime::engine::EngineAction;
use crate::runtime::queue::ActionQueue;
use crate::runtime::trigger::{TriggerExtra, TriggerFlags, Trigger};
use crate::runtime::timer::{TimerModel, TimerFlags};
use crate::runtime::vars::Variables;
use crate::map::plan::Planner;
use crate::map::node::{NodeMap, FilteredNodes};
use crate::map::edge::{EdgeMap, FilteredEdges};
use crate::map::mapper::Mapper;
use crate::map::path::PathCategory;
use crate::ui::line::Line;
use crate::ui::style::{Color, Style};
use crate::ui::UserOutput;
use std::time::Duration;
use std::sync::{Arc, Mutex};
use mlua::{Lua, ToLua};
use uuid::Uuid;
use rusqlite::Connection;

/// 初始化运行时
///
/// 1. 定义全局变量表，Lua脚本通过SetVariable()和GetVariable()函数
///    对其中的值进行设置和查询
/// 2. 定义Lua脚本引擎中的的核心函数
///    有一部分函数借鉴了MUSHClient的函数签名。
pub fn init_lua(lua: &Lua, vtb: &Variables, tmpq: &ActionQueue) -> Result<()> {
    log::info!("initializing lua runtime");
    let globals = lua.globals();

    // 初始化SetVariable函数
    let vars = vtb.clone();
    let set_variable = lua.create_function(move |_, (k, v): (String, String)| {
        log::trace!("SetVariable function called");
        vars.insert(k, v);
        Ok(())
    })?;
    register_function(&globals, "SetVariable", set_variable)?;

    // 初始化GetVariable函数
    let vars = vtb.clone();
    let get_variable = lua.create_function(move |_, k: String| {
        log::trace!("GetVariable function called");
        match vars.get(&k) {
            Some(v) => Ok(Some(v.to_owned())),
            None => Ok(None),
        }
    })?;
    register_function(&globals, "GetVariable", get_variable)?;

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
    register_function(&globals, "SwitchCodec", switch_codec)?;

    // 重写print函数
    let print = lua.create_function(move |_, _: ()| Ok(()))?;
    register_function(&globals, "print", print)?;

    // 初始化Send函数
    let queue = tmpq.clone();
    let send = lua.create_function(move |_, s: String| {
        log::trace!("Send function called");
        queue.push(EngineAction::ExecuteUserOutput(UserOutput::Cmd(s)));
        Ok(())
    })?;
    register_function(&globals, "Send", send)?;

    // 初始化Note函数
    let queue = tmpq.clone();
    let note = lua.create_function(move |_, s: String| {
        log::trace!("Note function called");
        // queue.send_styled_line(Line::fmt_note(s));
        queue.push(EngineAction::SendLineToUI(Line::fmt_note(s), None));
        Ok(())
    })?;
    register_function(&globals, "Note", note)?;

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
    register_function(&globals, "ColourNote", colour_note)?;

    // 初始化GetUniqueID函数
    let get_unique_id = lua.create_function(move |_, _: ()| {
        let id = Uuid::new_v4();
        Ok(id.to_simple().to_string())
    })?;
    register_function(&globals, "GetUniqueID", get_unique_id)?;

    // 别名常量
    let alias_flag: mlua::Table = lua.create_table()?;
    alias_flag.set("Enabled", 1)?;
    alias_flag.set("KeepEvaluating", 8)?;
    globals.set("alias_flag", alias_flag)?;

    // 别名回调注册表
    let alias_callbacks = lua.create_table()?;
    globals.set(engine::GLOBAL_ALIAS_CALLBACKS, alias_callbacks)?;

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
            let alias = Alias::builder()
                .name(name)
                .group(group)
                .pattern(pattern)?
                .extra(flags)
                .build();
            // 此处可以直接向Lua表中添加回调，因为：
            // 1. mlua::Function与Lua运行时同生命周期，不能在EventQueue中传递
            // 2. 需保证在下次检验名称时若重名则失败
            // 重要：在处理RuntimAction时，需清理曾添加的回调函数
            alias_callbacks.set(alias.name.to_owned(), func)?;
            queue.push(EngineAction::CreateAlias(alias));
            Ok(())
        },
    )?;
    register_function(&globals, "CreateAlias", create_alias)?;

    // 初始化DeleteAlias函数
    let queue = tmpq.clone();
    let delete_alias = lua.create_function(move |_, name: String| {
        log::trace!("DeleteAlias function called");
        queue.push(EngineAction::DeleteAlias(name));
        Ok(())
    })?;
    register_function(&globals, "DeleteAlias", delete_alias)?;

    // 触发器常量
    let trigger_flag: mlua::Table = lua.create_table()?;
    trigger_flag.set("Enabled", 1)?;
    trigger_flag.set("KeepEvaluating", 8)?;
    trigger_flag.set("OneShot", 32768)?;
    globals.set("trigger_flag", trigger_flag)?;

    // 触发器回调注册表
    let trigger_callbacks = lua.create_table()?;
    globals.set(engine::GLOBAL_TRIGGER_CALLBACKS, trigger_callbacks)?;

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
            let trigger = Trigger::builder()
                .name(name)
                .group(group)
                .pattern(pattern)?
                .extra(TriggerExtra { match_lines, flags })
                .build();
            // 同alias
            trigger_callbacks.set(trigger.name.to_owned(), func)?;
            queue.push(EngineAction::CreateTrigger(trigger));
            Ok(())
        },
    )?;
    register_function(&globals, "CreateTrigger", create_trigger)?;

    // 初始化DeleteTrigger函数
    let queue = tmpq.clone();
    let delete_trigger = lua.create_function(move |_, name: String| {
        log::trace!("DeleteTrigger function called");
        queue.push(EngineAction::DeleteTrigger(name));
        Ok(())
    })?;
    register_function(&globals, "DeleteTrigger", delete_trigger)?;

    // 初始化EnableTriggerGroup函数
    let queue = tmpq.clone();
    let enable_trigger_group = lua.create_function(move |_, (name, enabled): (String, bool)| {
        log::trace!("EnableTriggerGroup function called");
        queue.push(EngineAction::EnableTriggerGroup(name, enabled));
        Ok(())
    })?;
    register_function(&globals, "EnableTriggerGroup", enable_trigger_group)?;

    // 定时器常量
    let timer_flag: mlua::Table = lua.create_table()?;
    timer_flag.set("Enabled", 1)?;
    timer_flag.set("OneShot", 4)?;
    globals.set("timer_flag", timer_flag)?;

    // 定时器回调注册表
    let timer_callbacks = lua.create_table()?;
    globals.set(engine::GLOBAL_TIMER_CALLBACKS, timer_callbacks)?;

    // 初始化CreateTimer函数
    let queue = tmpq.clone();
    let create_timer = lua.create_function(
        move |lua, (name, group, tick_in_millis, flags, func): (String, String, u64, u16, mlua::Function)| {
            log::trace!("CreateTimer function called");
            let timer_callbacks: mlua::Table = 
                lua.globals().get(engine::GLOBAL_TIMER_CALLBACKS)?;
            let tick_time = Duration::from_millis(tick_in_millis);
            let flags = TimerFlags::from_bits(flags).ok_or_else(|| {
                mlua::Error::external(Error::RuntimeError(format!(
                    "invalid timer flags {}",
                    flags
                )))})?;
            let tm = TimerModel::new(name, group, tick_time, flags);
            // 同alias
            timer_callbacks.set(tm.name.to_owned(), func)?;
            queue.push(EngineAction::CreateTimer(tm));
            Ok(())
    })?;
    register_function(&globals, "CreateTimer", create_timer)?;

    // 初始化DeleteTimer函数
    let queue = tmpq.clone();
    let delete_timer = lua.create_function(
        move |_, name: String| {
            log::trace!("DeleteTimer function called");
            queue.push(EngineAction::DeleteTimer(name));
            Ok(())
    })?;
    register_function(&globals, "DeleteTimer", delete_timer)?;

    // 初始化EnableTimerGroup函数
    let queue = tmpq.clone();
    let enable_timer_group = lua.create_function(move |_, (name, enabled): (String, bool)| {
        log::trace!("EnableTimerGroup function called");
        queue.push(EngineAction::EnableTimerGroup(name, enabled));
        Ok(())
    })?;
    register_function(&globals, "EnableTimerGroup", enable_timer_group)?;

    // 初始化DoAfter函数
    let queue = tmpq.clone();
    let do_after = lua.create_function(move |lua, (tick_in_millis, func): (u64, mlua::Function)| {
        log::trace!("DoAfter function called");
        let timer_callbacks: mlua::Table = 
                lua.globals().get(engine::GLOBAL_TIMER_CALLBACKS)?;
        let tick_time = Duration::from_millis(tick_in_millis);
        let flags = TimerFlags::ENABLED | TimerFlags::ONESHOT;
        let name = Uuid::new_v4().to_simple().to_string();
        let tm = TimerModel::new(name, "TemporaryDoAfter", tick_time, flags);
        timer_callbacks.set(tm.name.to_owned(), func)?;
        queue.push(EngineAction::CreateTimer(tm));
        Ok(())
    })?;
    register_function(&globals, "DoAfter", do_after)?;

    // 初始化LoadFile函数
    let queue = tmpq.clone();
    let load_file = lua.create_function(move |_, path: String| {
        queue.push(EngineAction::LoadFile(path));
        Ok(())
    })?;
    register_function(&globals, "LoadFile", load_file)?;

    Ok(())
}

pub fn init_mapper(lua: &Lua, conn: Connection) -> Result<()> {
    log::info!("initializing mapper");
    let globals = lua.globals();

    let rooms = NodeMap::load_from_db(&conn)?;
    let paths = EdgeMap::load_from_db(&conn, &rooms)?;
    let rooms = Arc::new(rooms);
    let paths = Arc::new(paths);
    
    // 初始化FastWalk函数
    let planner = Planner::new(rooms.clone(), paths.clone());
    let fast_walk = lua.create_function(move |lua, (fromid, toid): (u32, u32)| {
        let plan = planner.walk(fromid, toid);
        plan.to_lua(lua)
    })?;
    register_function(&globals, "FastWalk", fast_walk)?;

    // 初始化Walk函数
    let planner = {
        let paths = FilteredEdges::new(paths.clone(), |p| p.category != PathCategory::Bus);
        Planner::new(rooms.clone(), paths.clone())
    };
    let walk = lua.create_function(move |lua, (fromid, toid): (u32, u32)| {
        let plan = planner.walk(fromid, toid);
        plan.to_lua(lua)
    })?;
    register_function(&globals, "Walk", walk)?;

    // 初始化traverse函数
    let planner = {
        let paths = FilteredEdges::new(paths.clone(), |p| {
            p.category != PathCategory::Bus && p.category != PathCategory::Boat
        });
        Planner::new(rooms.clone(), paths.clone())
    };
    let traverse = lua.create_function(move |lua, (centerid, depth): (u32, u32)| {
        let plan = planner.traverse(centerid, depth);
        plan.to_lua(lua)
    })?;
    register_function(&globals, "Traverse", traverse)?;

    let conn = Arc::new(Mutex::new(conn));

    // 初始化ListZones函数
    let mapper = Mapper::new(conn.clone());
    let list_zones = lua.create_function(move |lua, _: ()| {
        let zones = mapper.list_zones()?;
        zones.to_lua(lua)
    })?;
    register_function(&globals, "ListZones", list_zones)?;

    // 初始化GetZoneById函数
    let mapper = Mapper::new(conn.clone());
    let find_zone_by_id = lua.create_function(move |lua, id: u32| {
        match mapper.get_zone_by_id(id)? {
            Some(zone) => Ok(zone.to_lua(lua)?),
            None => Ok(mlua::Value::Nil),
        }
    })?;
    register_function(&globals, "GetZoneById", find_zone_by_id)?;

    // 初始化GetZoneByCode函数
    let mapper = Mapper::new(conn.clone());
    let find_zone_by_code = lua.create_function(move |lua, code: String| {
        match mapper.get_zone_by_code(&code)? {
            Some(zone) => Ok(zone.to_lua(lua)?),
            None => Ok(mlua::Value::Nil),
        }
    })?;
    register_function(&globals, "GetZoneByCode", find_zone_by_code)?;

    // 初始化GetZoneByName函数
    let mapper = Mapper::new(conn.clone());
    let find_zone_by_name = lua.create_function(move |lua, name: String| {
        match mapper.get_zone_by_name(&name)? {
            Some(zone) => Ok(zone.to_lua(lua)?),
            None => Ok(mlua::Value::Nil),
        }
    })?;
    register_function(&globals, "GetZoneByName", find_zone_by_name)?;

    // 初始化ListRoomsByZone函数
    let mapper = Mapper::new(conn.clone());
    let list_rooms_by_zone = lua.create_function(move |lua, zone: String| {
        let rooms = mapper.list_rooms_by_zone(&zone)?;
        if rooms.is_empty() {
            return Ok(mlua::Value::Nil);
        }
        Ok(rooms.to_lua(lua)?)
    })?;
    register_function(&globals, "ListRoomsByZone", list_rooms_by_zone)?;

    // 初始化ListRoomsByName函数
    let mapper = Mapper::new(conn.clone());
    let list_rooms_by_name = lua.create_function(move |lua, name: String| {
        let rooms = mapper.list_rooms_by_name(&name)?;
        if rooms.is_empty() {
            return Ok(mlua::Value::Nil);
        }
        Ok(rooms.to_lua(lua)?)
    })?;
    register_function(&globals, "ListRoomsByName", list_rooms_by_name)?;

    // 初始化ListRoomsByNameAndZone函数
    let mapper = Mapper::new(conn.clone());
    let list_rooms_by_name_and_zone = lua.create_function(move |lua, (name, zone): (String, String)| {
        let rooms = mapper.list_rooms_by_name_and_zone(&name, &zone)?;
        if rooms.is_empty() {
            return Ok(mlua::Value::Nil);
        }
        Ok(rooms.to_lua(lua)?)
    })?;
    register_function(&globals, "ListRoomsByNameAndZone", list_rooms_by_name_and_zone)?;

    // 初始化ListRoomsByDescription函数
    let mapper = Mapper::new(conn.clone());
    let list_rooms_by_description = lua.create_function(move |lua, description: String| {
        let rooms = mapper.list_rooms_by_description(&description)?;
        if rooms.is_empty() {
            return Ok(mlua::Value::Nil);
        }
        Ok(rooms.to_lua(lua)?)
    })?;
    register_function(&globals, "ListRoomsByDescription", list_rooms_by_description)?;

    // 初始化ListRoomsByNpc函数
    let mapper = Mapper::new(conn.clone());
    let list_rooms_by_npc = lua.create_function(move |lua, npc: String| {
        let rooms = mapper.list_rooms_by_npc(&npc)?;
        if rooms.is_empty() {
            return Ok(mlua::Value::Nil);
        }
        Ok(rooms.to_lua(lua)?)
    })?;
    register_function(&globals, "ListRoomsByNpc", list_rooms_by_npc)?;

    Ok(())
}

fn register_function<'lua>(namespace: &'lua mlua::Table, name: impl AsRef<str>, function: mlua::Function<'lua>) -> Result<()> {
    let name = name.as_ref();
    log::trace!("initializing function {}", name);
    namespace.set(name, function)?;
    Ok(())
}
