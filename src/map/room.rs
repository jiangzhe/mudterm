use crate::map::node::Node;
use rusqlite::{Result, Row};
use mlua::{Lua, ToLua, Value};
use mlua::Result as LuaResult;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Room {
    pub id: u32,
    pub name: String,
    pub code: String,
    pub description: String,
    pub exits: String,
    pub zone: String,
    pub mapinfo: String,
    pub blockzone: String,
}

impl Node for Room {
    fn id(&self) -> u32 {
        self.id
    }
}

impl Room {
    pub(crate) fn from_row(row: &Row) -> Result<Self> {
        Ok(Room {
            id: row.get(0)?,
            name: row.get(1)?,
            code: row.get(2)?,
            description: row.get(3)?,
            exits: row.get(4)?,
            zone: row.get(5)?,
            mapinfo: row.get(6)?,
            blockzone: row.get(7)?,
        })
    }
}

impl<'lua> ToLua<'lua> for Room {
    fn to_lua(self, lua: &'lua Lua) -> LuaResult<Value<'lua>> {
        ToLua::to_lua(&self, lua)
    }
}

impl<'lua> ToLua<'lua> for &Room {
    fn to_lua(self, lua: &'lua Lua) -> LuaResult<Value<'lua>> {
        let table = lua.create_table()?;
        table.set("id", self.id)?;
        table.set("name", &self.name[..])?;
        table.set("code", &self.code[..])?;
        table.set("description", &self.description[..])?;
        table.set("exits", &self.exits[..])?;
        table.set("zone", &self.zone[..])?;
        Ok(Value::Table(table))
    }
}