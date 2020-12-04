use std::collections::HashMap;
use crate::map::room::Room;
use rusqlite::{Result, Connection, params};
use std::sync::Arc;

pub trait Node: Clone {
    // 节点编号
    fn id(&self) -> u32;
}

pub trait Nodes {
    type Node: Node;

    fn get(&self, id: u32) -> Option<Self::Node>;

    fn contains(&self, id: u32) -> bool;
}

impl<T: Nodes> Nodes for Arc<T> {
    type Node = <T as Nodes>::Node;
    fn get(&self, id: u32) -> Option<Self::Node> {
        self.as_ref().get(id)
    }

    fn contains(&self, id: u32) -> bool {
        self.as_ref().contains(id)
    }
}

/// 房间缓存，从数据库加载而来
#[derive(Debug, Clone)]
pub struct NodeMap<N>(HashMap<u32, N>);

impl<N: Node> Nodes for NodeMap<N> {
    type Node = N;
    fn get(&self, id: u32) -> Option<N> {
        self.0.get(&id).cloned()
    }

    fn contains(&self, id: u32) -> bool {
        self.0.contains_key(&id)
    }
}

impl<N> NodeMap<N> {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
}

impl<N: Node> NodeMap<N> {
    #[cfg(test)]
    pub fn put(&mut self, node: N) -> Option<N> {
        self.0.insert(node.id(), node)
    }
}

impl NodeMap<Room> {
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

/// 支持筛选部分节点
#[derive(Debug, Clone)]
pub struct FilteredNodes<N, F> {
    map: Arc<NodeMap<N>>,
    filter: F,
}

impl<N, F> Nodes for FilteredNodes<N, F>
where
    N: Node,
    F: Fn(&N) -> bool,
    F: Clone,
{
    type Node = N;
    fn get(&self, id: u32) -> Option<N> {
        match self.map.get(id) {
            Some(node) if (self.filter)(&node) => Some(node),
            _ => None,
        }
    }

    fn contains(&self, id: u32) -> bool {
        match self.get(id) {
            Some(_) => true,
            None => false,
        }
    }
}

impl<N, F> FilteredNodes<N, F>
where
    N: Node,
    F: Fn(&N) -> bool,
    F: Clone,
{
    pub fn new(map: Arc<NodeMap<N>>, filter: F) -> Self {
        Self{map, filter}
    }
}