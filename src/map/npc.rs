use crate::map::{FromRow, Nodes};
use rusqlite::{params, Connection, Result, Row};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Npc {
    pub id: String,
    pub name: String,
    pub roomid: u32,
    pub zone: String,
}

impl FromRow for Npc {
    fn from_row(row: &Row) -> Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            name: row.get(1)?,
            roomid: row.get(2)?,
            zone: row.get(3)?,
        })
    }
}

pub struct Npcs(HashMap<String, Vec<Npc>>);

impl Npcs {
    pub fn load_from_db<N>(conn: &Connection, rooms: &Nodes<N>) -> Result<Self> {
        let mut stmt = conn.prepare("SELECT * FROM npcs")?;
        let npcs_iter = stmt.query_map(params![], |row| Npc::from_row(row))?;
        let mut rs = HashMap::new();
        for npc in npcs_iter {
            let npc = npc?;
            if rooms.contains(npc.roomid) {
                rs.entry(npc.name.to_owned())
                    .or_insert_with(|| vec![])
                    .push(npc);
            }
        }
        Ok(Self(rs))
    }
}
