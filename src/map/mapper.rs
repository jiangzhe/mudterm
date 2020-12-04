use rusqlite::{Connection, params};
use crate::error::Result;
use crate::map::room::Room;
use crate::map::npc::Npc;
use crate::map::zone::Zone;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct Mapper(Arc<Mutex<Connection>>);

impl Mapper {

    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self(conn)
    }

    pub fn load_from_file(filepath: &str) -> Result<Self> {
        let conn = Connection::open(filepath)?;
        Ok(Self(Arc::new(Mutex::new(conn))))
    }

    pub fn list_rooms_by_zone(&self, zone: &str) -> Result<Vec<Room>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn
            .prepare_cached("SELECT * FROM rooms WHERE zone = ?1")?;
        let room_iter = stmt.query_map(params![zone], |row| Room::from_row(row))?;
        let mut rooms = Vec::new();
        for room in room_iter {
            rooms.push(room?);
        }
        Ok(rooms)
    }

    pub fn list_rooms_by_name_and_zone(&self, name: &str, zone: &str) -> Result<Vec<Room>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn
            .prepare_cached("SELECT * FROM rooms WHERE name = ?1 and zone = ?2")?;
        let room_iter = stmt.query_map(params![name, zone], |row| Room::from_row(row))?;
        let mut rooms = Vec::new();
        for room in room_iter {
            rooms.push(room?);
        }
        Ok(rooms)
    }

    pub fn list_rooms_by_name(&self, name: &str) -> Result<Vec<Room>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn
            .prepare_cached("SELECT * FROM rooms WHERE name = ?1")?;
        let room_iter = stmt.query_map(params![name], |row| Room::from_row(row))?;
        let mut rooms = Vec::new();
        for room in room_iter {
            rooms.push(room?);
        }
        Ok(rooms)
    }

    pub fn list_rooms_by_description(&self, description: &str) -> Result<Vec<Room>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn
            .prepare(&format!("SELECT * FROM rooms WHERE description LIKE '%{}%'", description))?;
        let room_iter = stmt.query_map(params![], |row| Room::from_row(row))?;
        let mut rooms = Vec::new();
        for room in room_iter {
            rooms.push(room?);
        }
        Ok(rooms)
    }

    pub fn list_rooms_by_npc(&self, npc_name: &str) -> Result<Vec<Room>> {
        let conn = self.0.lock().unwrap();
        let mut npc_stmt = conn
            .prepare_cached("SELECT * FROM npcs WHERE name = ?1")?;
        let npc_iter = npc_stmt.query_map(params![npc_name], |row| Npc::from_row(row))?;
        let mut room_ids = HashSet::new();
        let mut rooms = Vec::new();
        for npc in npc_iter {
            let npc = npc?;
            room_ids.insert(npc.roomid);
        }
        let conn = self.0.lock().unwrap();
        let mut room_stmt = conn
            .prepare_cached("SELECT * FROM rooms WHERE id = ?1")?;
        for room_id in room_ids {
            let room_iter = room_stmt.query_map(params![room_id], |row| Room::from_row(row))?;
            for room in room_iter {
                rooms.push(room?);
            }
        }
        Ok(rooms)
    }

    pub fn list_zones(&self) -> Result<Vec<Zone>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn
            .prepare_cached("SELECT * FROM zones")?;
        let zone_iter = stmt.query_map(params![], |row| Zone::from_row(row))?;
        let mut zones = Vec::new();
        for zone in zone_iter {
            zones.push(zone?);
        }
        Ok(zones)
    }

    pub fn get_zone_by_id(&self, zoneid: u32) -> Result<Option<Zone>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn
            .prepare_cached("SELECT * FROM zones WHERE id = ?1")?;
        let zone_iter = stmt.query_map(params![zoneid], |row| Zone::from_row(row))?;
        for zone in zone_iter {
            return Ok(Some(zone?));
        }
        Ok(None)
    }

    pub fn get_zone_by_code(&self, zonecode: &str) -> Result<Option<Zone>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn
            .prepare_cached("SELECT * FROM zones WHERE code = ?1")?;
        let zone_iter = stmt.query_map(params![zonecode], |row| Zone::from_row(row))?;
        for zone in zone_iter {
            return Ok(Some(zone?));
        }
        Ok(None)
    }

    pub fn get_zone_by_name(&self, zonename: &str) -> Result<Option<Zone>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn
            .prepare_cached("SELECT * FROM zones WHERE name = ?1")?;
        let zone_iter = stmt.query_map(params![zonename], |row| Zone::from_row(row))?;
        for zone in zone_iter {
            return Ok(Some(zone?));
        }
        Ok(None)
    }
}
