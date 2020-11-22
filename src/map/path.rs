use crate::map::{Edge, FromRow};
use rusqlite::{Result, Row};

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

impl FromRow for Path {
    fn from_row(row: &Row) -> Result<Self> {
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
