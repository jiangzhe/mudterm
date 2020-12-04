use crate::map::edge::Edge;
use rusqlite::{Result, Row};
use mlua::{Lua, ToLua, Value};
use mlua::Result as LuaResult;

#[derive(Debug, Clone)]
pub struct Path {
    pub startid: u32,
    pub endid: u32,
    pub path: String,
    pub endcode: String,
    pub weight: u32,
    pub enabled: bool,
    pub category: PathCategory,
    pub mapchange: bool,
    pub blockers: String,
}

impl Edge for Path {
    fn pseudo(id: u32) -> Path {
        Self {
            startid: id,
            endid: id,
            path: String::from("look"),
            endcode: String::new(),
            weight: 0,
            enabled: true,
            category: PathCategory::Normal,
            mapchange: false,
            blockers: String::new(),
        }
    }

    fn startid(&self) -> u32 {
        self.startid
    }

    fn endid(&self) -> u32 {
        self.endid
    }

    fn weight(&self) -> u32 {
        self.weight
    }
}

impl Path {
    pub(crate) fn from_row(row: &Row) -> Result<Self> {
        Ok(Self {
            startid: row.get(0)?,
            endid: row.get(1)?,
            path: row.get(2)?,
            endcode: row.get(3)?,
            weight: row.get(4)?,
            enabled: row.get(5)?,
            category: PathCategory::from(row.get::<_, u32>(6)?),
            mapchange: row.get(7)?,
            blockers: row.get(8)?,
        })
    }
}

impl<'lua> ToLua<'lua> for Path {
    fn to_lua(self, lua: &'lua Lua) -> LuaResult<Value<'lua>> {
        ToLua::to_lua(&self, lua)
    }
}

impl<'lua> ToLua<'lua> for &Path {
    fn to_lua(self, lua: &'lua Lua) -> LuaResult<Value<'lua>> {
        let table = lua.create_table()?;
        table.set("startid", self.startid)?;
        table.set("endid", self.endid)?;
        table.set("path", &self.path[..])?;
        table.set("endcode", &self.endcode[..])?;
        // table.set("weight", self.weight)?;
        // table.set("enabled", self.enabled)?;
        table.set("category", self.category.to_string())?;
        table.set("mapchange", self.mapchange)?;
        table.set("blockers", &self.blockers[..])?;
        Ok(Value::Table(table))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathCategory {
    Normal,
    Multiple,
    Busy,
    Boat,
    Pause,
    Block,
    CheckBusy,
    Bus,
}

impl From<u32> for PathCategory {
    fn from(src: u32) -> Self {
        match src {
            1 => Self::Normal,
            2 => Self::Multiple,
            3 => Self::Busy,
            4 => Self::Boat,
            5 => Self::Pause,
            6 => Self::Block,
            7 => Self::CheckBusy,
            8 => Self::Bus,
            _ => Self::Normal,
        }
    }
}

impl<'a> From<&'a str> for PathCategory {
    fn from(src: &str) -> Self {
        match src {
            "normal" => Self::Normal,
            "multiple" => Self::Multiple,
            "busy" => Self::Busy,
            "boat" => Self::Boat,
            "pause" => Self::Pause,
            "block" => Self::Block,
            "checkbusy" => Self::CheckBusy,
            "bus" => Self::Bus,
            _ => Self::Normal,
        }
    }
}

impl ToString for PathCategory {
    fn to_string(&self) -> String {
        let s = match self {
            Self::Normal => "normal",
            Self::Multiple => "multiple",
            Self::Busy => "busy",
            Self::Boat => "boat",
            Self::Pause => "pause",
            Self::Block => "block",
            Self::CheckBusy => "checkbusy",
            Self::Bus => "bus",
        };
        s.to_owned()
    }
}