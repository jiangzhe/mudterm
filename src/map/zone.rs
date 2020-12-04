use rusqlite::{Row, Result};
use mlua::{Lua, Value, ToLua};
use mlua::Result as LuaResult;

#[derive(Debug, Clone)]
pub struct Zone {
    pub id: String,
    pub code: String,
    pub name: String,
    pub centercode: String,
}

impl Zone {
    pub fn from_row(row: &Row) -> Result<Self> {
        let id = row.get(0)?;
        let code = row.get(1)?;
        let name = row.get(2)?;
        let centercode = row.get(3)?;
        Ok(Self{id, code, name, centercode})
    }
}

impl<'lua> ToLua<'lua> for &Zone {
    fn to_lua(self, lua: &'lua Lua) -> LuaResult<Value<'lua>> {
        let table = lua.create_table()?;
        table.set("id", &self.id[..])?;
        table.set("code", &self.code[..])?;
        table.set("name", &self.name[..])?;
        table.set("centercode", &self.centercode[..])?;
        Ok(Value::Table(table))
    }
}

impl<'lua> ToLua<'lua> for Zone {
    fn to_lua(self, lua: &'lua Lua) -> LuaResult<Value<'lua>> {
        ToLua::to_lua(&self, lua)
    }
}