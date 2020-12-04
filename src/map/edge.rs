use std::collections::HashMap;
use rusqlite::{Result, Connection, params};
use crate::map::path::Path;
use crate::map::node::Nodes;
use std::sync::Arc;

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

pub trait Edges {
    type Edge: Edge;
    // 查询出口
    fn exits(&self, id: u32) -> Vec<&Self::Edge>;
}

impl<T: Edges> Edges for Arc<T> {
    type Edge = <T as Edges>::Edge;

    fn exits(&self, id: u32) -> Vec<&Self::Edge> {
        self.as_ref().exits(id)
    }
}

/// 基于HashMap的默认实现
#[derive(Debug, Clone)]
pub struct EdgeMap<E>(HashMap<u32, Vec<E>>);

impl<E: Edge> Edges for EdgeMap<E> {
    type Edge = E;
    fn exits(&self, id: u32) -> Vec<&E> {
        self.exits_slice(id).iter().collect()
    }
}

impl<E> EdgeMap<E> {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    fn exits_slice(&self, id: u32) -> &[E] {
        self.0.get(&id)
            .map(|r| &r[..])
            .unwrap_or_default()
    }
}

impl<E: Edge> EdgeMap<E> {
    #[cfg(test)]
    pub fn insert(&mut self, edge: E) {
        self.0
            .entry(edge.startid())
            .or_insert_with(|| vec![])
            .push(edge.clone());
    }
}

impl EdgeMap<Path> {
    pub fn load_from_db<NS: Nodes>(conn: &Connection, nodes: &NS) -> Result<Self> {
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

/// 支持筛选部分路径
#[derive(Debug, Clone)]
pub struct FilteredEdges<E, F> {
    map: Arc<EdgeMap<E>>,
    filter: F,
}

impl<E, F> Edges for FilteredEdges<E, F>
where
    E: Edge,
    F: Fn(&E) -> bool,
    F: Clone,
{
    type Edge = E;
    fn exits(&self, id: u32) -> Vec<&E> {
        self.map.exits_slice(id)
            .iter()
            .filter(|e| (self.filter)(e))
            .collect()
    }
}

impl<E, F> FilteredEdges<E, F>
where
    E: Edge,
    F: Fn(&E) -> bool,
    F: Clone,
{
    pub fn new(map: Arc<EdgeMap<E>>, filter: F) -> Self {
        Self{map, filter}
    }
}