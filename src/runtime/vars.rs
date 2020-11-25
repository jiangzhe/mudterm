use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, RwLock};

/// 脚本环境中的变量存储和查询
#[derive(Debug, Clone)]
pub struct Variables(Arc<RwLock<HashMap<String, String>>>);

impl Variables {
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(HashMap::new())))
    }

    pub fn get<Q>(&self, name: &Q) -> Option<String>
    where
        String: Borrow<Q>,
        Q: Hash + Eq,
    {
        let m = self.0.read().unwrap();
        m.get(name).map(|s| s.to_owned())
    }

    pub fn insert(&self, name: String, value: String) -> Option<String> {
        let mut m = self.0.write().unwrap();
        m.insert(name, value)
    }
}
