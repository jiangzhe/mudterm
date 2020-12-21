use crate::error::{Error, Result};
use mlua::ToLua;
use regex::Regex;
use std::collections::HashMap;
use lazy_static::lazy_static;

/// 持有模型的基本属性
#[derive(Debug, Clone)]
pub struct Model<X> {
    pub name: String,
    pub group: String,
    pub pattern: String,
    pub enabled: bool,
    pub extra: X,
    pub(super) re: Regex,
}

impl<X: PartialEq> PartialEq for Model<X> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name &&
        self.group == other.group &&
        self.pattern == other.pattern &&
        self.enabled == other.enabled &&
        self.extra == other.extra
    }
}

pub struct ModelBuilder<X> {
    name: String,
    group: String,
    pattern: String,
    enabled: bool,
    extra: X,
    re: Regex,
}

lazy_static! {
    static ref EMPTY_REGEX: Regex = Regex::new("").unwrap();
}

impl<X: Default> Default for ModelBuilder<X> {
    fn default() -> Self {
        Self{
            re: EMPTY_REGEX.clone(),
            ..Default::default()
        }
    }
}

impl<X> ModelBuilder<X> {

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = group.into();
        self
    }

    pub fn pattern(mut self, pattern: impl Into<String>) -> Result<Self> {
        let pattern = pattern.into();
        let re = Regex::new(&pattern)?;
        self.pattern = pattern;
        self.re = re;
        Ok(self)
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn extra(mut self, extra: X) -> Self {
        self.extra = extra;
        self
    }

    pub fn build(self) -> Model<X> {
        Model{
            name: self.name,
            group: self.group,
            pattern: self.pattern,
            enabled: self.enabled,
            extra: self.extra,
            re: self.re,
        }
    }
}

impl<X: Default> Model<X> {
    pub fn builder() -> ModelBuilder<X> {
        ModelBuilder::default()
    }
}

impl<X> Model<X> {

    pub fn captures(&self, input: &str) -> Result<ModelCaptures> {
        let captures = self.re.captures(input.as_ref()).ok_or_else(|| {
            Error::RuntimeError(format!(
                "mismatch alias[name={}, pattern={}]",
                &self.name, &self.pattern
            ))
        })?;
        let mut names = self.re.capture_names().skip(1).fuse();
        let mut mapping = HashMap::new();
        let mut captures = captures.iter().enumerate().skip(1);
        while let Some((i, om)) = captures.next() {
            if let Some(m) = om {
                if let Some(name) = names.next() {
                    if let Some(name) = name {
                        mapping.insert(NumberOrString::new_string(name), m.as_str().to_owned());
                    }
                }
                mapping.insert(NumberOrString::Number(i), m.as_str().to_owned());
            }
        }
        Ok(ModelCaptures(mapping))
    }
}

// is_match定义输入是否匹配模型
pub trait ModelMatch {
    type Input: ?Sized;
    fn is_match(&self, input: &Self::Input) -> bool;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NumberOrString {
    Number(usize),
    String(String),
}

impl NumberOrString {
    pub fn new_string(s: impl Into<String>) -> Self {
        Self::String(s.into())
    }

    pub fn new_number(n: usize) -> Self {
        Self::Number(n)
    }
}

#[derive(Debug, Clone)]
pub struct ModelCaptures(HashMap<NumberOrString, String>);

impl<'lua> ToLua<'lua> for ModelCaptures {
    fn to_lua(self, lua: &'lua mlua::Lua) -> mlua::Result<mlua::Value<'lua>> {
        let table = lua.create_table()?;
        for (k, v) in self.0 {
            match k {
                NumberOrString::Number(n) => table.set(n, v)?,
                NumberOrString::String(s) => table.set(s, v)?,
            }
        }
        Ok(mlua::Value::Table(table))
    }
}

pub trait ModelStore<M> 
where
    M: ModelMatch,
{
    /// 添加模型，出错则返回原值
    fn add(&mut self, mx: M) -> std::result::Result<(), M>;

    /// 删除模型
    fn remove(&mut self, name: impl AsRef<str>) -> Option<M>;

    /// 启用模型
    fn enable(&mut self, name: impl AsRef<str>, enabled: bool) -> Option<&M>;

    /// 禁用模型
    fn enable_group(&mut self, group: impl AsRef<str>, enabled: bool) -> usize;

    /// 查询单个模型
    fn get(&self, name: impl AsRef<str>) -> Option<&M>;

    /// 查询单个模型，可修改
    fn get_mut(&mut self, name: impl AsRef<str>) -> Option<&mut M>;

    /// 模型数目
    fn len(&self) -> usize;

    /// 匹配输出第一个匹配到的模型
    fn match_first(&self, input: &M::Input) -> Option<&M>;

    /// 输出所有匹配成功的模型
    fn match_all(&self, input: &M::Input) -> Vec<&M>;
}

#[derive(Debug)]
pub struct MapModelStore<M>(pub(super) HashMap<String, M>);

impl<M> MapModelStore<M> 
where
    M: ModelMatch,
{
    pub fn new() -> Self {
        Self(HashMap::new())
    }
}

impl<X> ModelStore<Model<X>> for MapModelStore<Model<X>>
where
    Model<X>: ModelMatch,
{
    fn add(&mut self, m: Model<X>) -> std::result::Result<(), Model<X>> {
        if self.0.contains_key(&m.name) {
            return Err(m);
        }
        self.0.insert(m.name.to_owned(), m);
        Ok(())
    }

    fn remove(&mut self, name: impl AsRef<str>) -> Option<Model<X>> {
        self.0.remove(name.as_ref())
    }

    fn enable(&mut self, name: impl AsRef<str>, enabled: bool) -> Option<&Model<X>> {
        if let Some(m) = self.get_mut(name.as_ref()) {
            m.enabled = enabled;
            return Some(m);
        }
        None
    }

    fn enable_group(&mut self, group: impl AsRef<str>, enabled: bool) -> usize {
        let group = group.as_ref();
        let mut n = 0;
        for m in self.0.values_mut() {
            if group == m.group {
                m.enabled = enabled;
                n += 1;
            }
        }
        n
    }

    fn get(&self, name: impl AsRef<str>) -> Option<&Model<X>> {
        self.0.get(name.as_ref())
    }

    fn get_mut(&mut self, name: impl AsRef<str>) -> Option<&mut Model<X>> {
        self.0.get_mut(name.as_ref())
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn match_first(&self, input: &<Model<X> as ModelMatch>::Input) -> Option<&Model<X>> {
        for m in self.0.values() {
            if m.enabled && m.is_match(input) {
                return Some(m);
            }
        }
        None
    }

    fn match_all(&self, input: &<Model<X> as ModelMatch>::Input) -> Vec<&Model<X>> {
        let mut rs = vec![];
        for m in self.0.values() {
            if m.enabled && m.is_match(input) {
                rs.push(m);
            }
        }
        rs
    }
}

#[derive(Debug)]
pub struct VecModelStore<M>(pub(super) Vec<M>);

impl<X> VecModelStore<Model<X>> {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    #[inline]
    fn get_idx(&self, name: impl AsRef<str>) -> Option<usize> {
        let name = name.as_ref();
        if name.is_empty() {
            return None;
        }
        self.0.iter().position(|m| &m.name == name)
    }
}

impl<X> ModelStore<Model<X>> for VecModelStore<Model<X>>
where
    Model<X>: ModelMatch,
{
    fn add(&mut self, m: Model<X>) -> std::result::Result<(), Model<X>> {
        let opt = if m.name.is_empty() {
            None
        } else {
            self.get(&m.name)
        };
        match opt {
            None => self.0.push(m),
            Some(_) => {
                return Err(m);
            }
        }
        Ok(())
    }

    fn remove(&mut self, name: impl AsRef<str>) -> Option<Model<X>> {
        let name = name.as_ref();
        if name.is_empty() {
            return None;
        }
        match self.get_idx(name) {
            None => None,
            Some(idx) => Some(self.0.swap_remove(idx)),
        }
    }

    fn enable(&mut self, name: impl AsRef<str>, enabled: bool) -> Option<&Model<X>> {
        let name = name.as_ref();
        if name.is_empty() {
            return None;
        }
        match self.get_mut(&name) {
            None => None,
            Some(m) => {
                m.enabled = enabled;
                Some(m)
            }
        }
    }

    fn enable_group(&mut self, group: impl AsRef<str>, enabled: bool) -> usize {
        let group = group.as_ref();
        if group.is_empty() {
            return 0;
        }
        let mut n = 0;
        for me in self.0.iter_mut() {
            if &me.group == group {
                me.enabled = enabled;
                n += 1;
            }
        }
        n
    }

    fn get(&self, name: impl AsRef<str>) -> Option<&Model<X>> {
        let name = name.as_ref();
        if name.is_empty() {
            return None;
        }
        self.0.iter().find(|me| &me.name == name)
    }

    fn get_mut(&mut self, name: impl AsRef<str>) -> Option<&mut Model<X>> {
        let name = name.as_ref();
        if name.is_empty() {
            return None;
        }
        self.0.iter_mut().find(|me| &me.name == name)
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn match_first(&self, input: &<Model<X> as ModelMatch>::Input) -> Option<&Model<X>> {
        for me in &self.0 {
            if me.enabled && me.is_match(input) {
                return Some(me);
            }
        }
        None
    }

    fn match_all(&self, input: &<Model<X> as ModelMatch>::Input) -> Vec<&Model<X>> {
        let mut rs = vec![];
        for m in &self.0 {
            if m.enabled && m.is_match(input) {
                rs.push(m);
            }
        }
        rs
    }
}
