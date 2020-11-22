use crate::map::{FromRow, Node};
use rusqlite::{Result, Row};

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

impl FromRow for Room {
    fn from_row(row: &Row) -> Result<Self> {
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
