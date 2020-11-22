pub mod npc;
pub mod path;
pub mod plan;
pub mod room;

// use crate::error::Result;
use npc::Npcs;
use path::Path;
use plan::Planner;
use room::Room;
use rusqlite::{params, Connection, Result, Row};
use std::collections::HashMap;

pub trait FromRow: Sized {
    // 转化自sqlite的行
    fn from_row(row: &Row) -> Result<Self>;
}

pub trait Node {
    // 节点编号
    fn id(&self) -> u32;
}

pub trait Edge: Clone + Sized {
    // 创建一条虚拟边，其中终点都是传入id
    fn pseudo(id: u32) -> Self;

    // 起始节点编号
    fn startid(&self) -> u32;

    // 结束节点编号
    fn endid(&self) -> u32;

    // 路径权重
    fn weight(&self) -> u32;
}

#[derive(Debug, Clone)]
pub struct Edges<E>(HashMap<u32, Vec<E>>);

impl<E> Edges<E> {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
}

impl<E: Edge> Edges<E> {
    #[cfg(test)]
    pub fn insert(&mut self, edge: E) {
        self.0
            .entry(edge.startid())
            .or_insert_with(|| vec![])
            .push(edge.clone());
    }

    pub fn exits(&self, id: u32) -> &[E] {
        self.0
            .get(&id)
            .map(|r| r.as_ref())
            .unwrap_or(<&[E]>::default())
    }
}

impl Edges<Path> {
    pub fn load_from_db<N>(conn: &Connection, nodes: &Nodes<N>) -> Result<Self> {
        let mut stmt = conn.prepare("SELECT * FROM paths")?;
        let path_iter = stmt.query_map(params![], |row| Path::from_row(row))?;
        let mut exits = HashMap::new();
        for path in path_iter {
            let path = path?;
            // 排除所有不可达路径
            if nodes.contains(path.startid) && nodes.contains(path.endid) {
                exits
                    .entry(path.startid)
                    .or_insert_with(|| vec![])
                    .push(path.clone());
            }
        }
        Ok(Self(exits))
    }
}

/// 房间缓存，从数据库加载而来
#[derive(Debug, Clone)]
pub struct Nodes<N>(HashMap<u32, N>);

impl<N> Nodes<N> {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn get(&self, id: u32) -> Option<&N> {
        self.0.get(&id)
    }

    pub fn contains(&self, id: u32) -> bool {
        self.0.contains_key(&id)
    }
}

impl<N: Node> Nodes<N> {
    #[cfg(test)]
    pub fn put(&mut self, node: N) -> Option<N> {
        self.0.insert(node.id(), node)
    }
}

impl Nodes<Room> {
    pub fn load_from_db(conn: &Connection) -> Result<Self> {
        let mut stmt = conn.prepare("SELECT * FROM rooms where name <> '' and zone <> ''")?;
        let room_iter = stmt.query_map(params![], |row| Room::from_row(row))?;
        let mut rs = HashMap::new();
        for room in room_iter {
            let room = room?;
            rs.insert(room.id, room);
        }
        Ok(Self(rs))
    }
}

pub struct Mapper {
    conn: Connection,
    pub rooms: Nodes<Room>,
    pub paths: Edges<Path>,
    pub npcs: Npcs,
}

impl Mapper {
    pub fn load_from_file(filepath: &str) -> Result<Self> {
        let conn = Connection::open(filepath)?;
        Self::load_from_conn(conn)
    }

    pub fn load_from_conn(conn: Connection) -> Result<Self> {
        let rooms = Nodes::load_from_db(&conn)?;
        let paths = Edges::load_from_db(&conn, &rooms)?;
        let npcs = Npcs::load_from_db(&conn, &rooms)?;
        Ok(Self {
            conn,
            rooms,
            paths,
            npcs,
        })
    }

    pub fn find_rooms_by_name_and_zone(&self, name: &str, zone: &str) -> Result<Vec<Room>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT * FROM rooms WHERE name = ?1 and zone = ?2")?;
        let room_iter = stmt.query_map(params![name, zone], |row| Room::from_row(row))?;
        let mut rooms = Vec::new();
        for room in room_iter {
            rooms.push(room?);
        }
        Ok(rooms)
    }

    pub fn planner(&self) -> Planner<Room, Path> {
        Planner::new(&self.rooms, &self.paths)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RoomWithDepth {
    id: u32,
    depth: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_planner_real_walk() {
        let mapper = Mapper::load_from_file("data/pkuxkx-utf8.db").unwrap();
        let planner = mapper.planner();
        let mut plan = planner.walk(1, 500);
        plan.reverse();
        println!("{:#?}", plan);
    }
}
