use crate::error::{Error, Result};
use mlua::ToLua;
use regex::Regex;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Model<X> {
    pub name: String,
    pub group: String,
    pub pattern: String,
    pub extra: X,
}

pub trait ModelExtra {
    type Input: AsRef<str>;

    fn enabled(&self) -> bool;

    fn set_enabled(&mut self, enabled: bool);

    fn keep_evaluating(&self) -> bool;

    fn set_keep_evaluating(&mut self, keep_evaluating: bool);

    fn is_match(&self, input: &Self::Input) -> bool;
}

/// 抽象对正则与脚本的编译
#[derive(Debug, Clone)]
pub struct ModelExec<M> {
    pub model: M,
    re: Regex,
}

impl<M: PartialEq> PartialEq for ModelExec<M> {
    fn eq(&self, other: &Self) -> bool {
        self.model == other.model
    }
}


impl<X: ModelExtra> ModelExec<Model<X>> {
    pub fn new(model: Model<X>, re: Regex) -> Self {
        Self { model, re }
    }

    pub fn is_match(&self, input: &X::Input) -> bool {
        self.model.extra.is_match(input) && self.re.is_match(input.as_ref())
    }

    // todo: 增加style的捕获
    pub fn captures(&self, input: impl AsRef<str>) -> Result<ModelCaptures> {
        let captures = self.re.captures(input.as_ref()).ok_or_else(|| {
            Error::RuntimeError(format!(
                "mismatch alias[name={}, pattern={}]",
                &self.model.name, &self.model.pattern
            ))
        })?;
        let mut names = self.re.capture_names().skip(1).fuse();
        let mut mapping = HashMap::new();
        let mut captures = captures.iter().enumerate().skip(1);
        while let Some((i, om)) = captures.next() {
            // let name = names.next().unwrap();
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

pub trait ModelStore<X, I>
where
    X: ModelExtra<Input=I>,
    I: AsRef<str>,
{
    /// 添加模型，出错则返回原值
    fn add(&mut self, me: ModelExec<Model<X>>) -> std::result::Result<(), ModelExec<Model<X>>>;

    /// 删除模型
    fn remove(&mut self, name: impl AsRef<str>) -> Option<ModelExec<Model<X>>>;

    /// 启用模型
    fn enable(&mut self, name: impl AsRef<str>, enabled: bool) -> Option<&ModelExec<Model<X>>>;

    /// 禁用模型
    fn enable_group(&mut self, group: impl AsRef<str>, enabled: bool) -> usize;

    /// 查询单个模型
    fn get(&self, name: impl AsRef<str>) -> Option<&ModelExec<Model<X>>>;

    /// 查询单个模型，可修改
    fn get_mut(&mut self, name: impl AsRef<str>) -> Option<&mut ModelExec<Model<X>>>;

    /// 模型数目
    fn len(&self) -> usize;

    /// 匹配输出第一个匹配到的模型
    fn match_first(&self, input: &I) -> Option<&ModelExec<Model<X>>>;

    /// 输出所有匹配成功的模型
    fn match_all(&self, input: &I) -> Vec<&ModelExec<Model<X>>>;
}

#[derive(Debug)]
pub struct MapModelStore<M>(pub(crate) HashMap<String, M>);

impl<X> MapModelStore<ModelExec<Model<X>>> {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
}

impl<X, I> ModelStore<X, I> for MapModelStore<ModelExec<Model<X>>>
where
    X: ModelExtra<Input=I>,
    I: AsRef<str>,
{
    fn add(&mut self, me: ModelExec<Model<X>>) -> std::result::Result<(), ModelExec<Model<X>>> {
        if self.0.contains_key(&me.model.name) {
            return Err(me);
        }
        self.0.insert(me.model.name.to_owned(), me);
        Ok(())
    }

    fn remove(&mut self, name: impl AsRef<str>) -> Option<ModelExec<Model<X>>> {
        self.0.remove(name.as_ref())
    }

    fn enable(&mut self, name: impl AsRef<str>, enabled: bool) -> Option<&ModelExec<Model<X>>> {
        if let Some(me) = self.get_mut(name.as_ref()) {
            me.model.extra.set_enabled(enabled);
            return Some(me);
        }
        None
    }

    fn enable_group(&mut self, group: impl AsRef<str>, enabled: bool) -> usize {
        let group = group.as_ref();
        let mut n = 0;
        for me in self.0.values_mut() {
            if group == me.model.group {
                me.model.extra.set_enabled(enabled);
                n += 1;
            }
        }
        n
    }

    fn get(&self, name: impl AsRef<str>) -> Option<&ModelExec<Model<X>>> {
        self.0.get(name.as_ref())
    }

    fn get_mut(&mut self, name: impl AsRef<str>) -> Option<&mut ModelExec<Model<X>>> {
        self.0.get_mut(name.as_ref())
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn match_first(&self, input: &I) -> Option<&ModelExec<Model<X>>> {
        for me in self.0.values() {
            if me.model.extra.enabled() && me.is_match(input) {
                return Some(me);
            }
        }
        None
    }

    fn match_all(&self, input: &I) -> Vec<&ModelExec<Model<X>>> {
        let mut rs = vec![];
        for me in self.0.values() {
            if me.model.extra.enabled() && me.is_match(input) {
                rs.push(me);
            }
        }
        rs
    }
}

#[derive(Debug)]
pub struct VecModelStore<ME>(pub(crate) Vec<ME>);

impl<X> VecModelStore<ModelExec<Model<X>>> {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    #[inline]
    fn get_idx(&self, name: impl AsRef<str>) -> Option<usize> {
        let name = name.as_ref();
        if name.is_empty() {
            return None;
        }
        self.0.iter().position(|me| &me.model.name == name)
    }
}

impl<X, I> ModelStore<X, I> for VecModelStore<ModelExec<Model<X>>>
where
    X: ModelExtra<Input=I>,
    I: AsRef<str>,
{
    fn add(&mut self, me: ModelExec<Model<X>>) -> std::result::Result<(), ModelExec<Model<X>>> {
        let opt = if me.model.name.is_empty() {
            None
        } else {
            self.get(&me.model.name)
        };
        match opt {
            None => self.0.push(me),
            Some(_) => {
                return Err(me);
            }
        }
        Ok(())
    }

    fn remove(&mut self, name: impl AsRef<str>) -> Option<ModelExec<Model<X>>> {
        let name = name.as_ref();
        if name.is_empty() {
            return None;
        }
        match self.get_idx(name) {
            None => None,
            Some(idx) => Some(self.0.swap_remove(idx)),
        }
    }

    fn enable(&mut self, name: impl AsRef<str>, enabled: bool) -> Option<&ModelExec<Model<X>>> {
        let name = name.as_ref();
        if name.is_empty() {
            return None;
        }
        match self.get_mut(&name) {
            None => None,
            Some(me) => {
                me.model.extra.set_enabled(enabled);
                Some(me)
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
            if &me.model.group == group {
                me.model.extra.set_enabled(enabled);
                n += 1;
            }
        }
        n
    }

    fn get(&self, name: impl AsRef<str>) -> Option<&ModelExec<Model<X>>> {
        let name = name.as_ref();
        if name.is_empty() {
            return None;
        }
        self.0.iter().find(|me| &me.model.name == name)
    }

    fn get_mut(&mut self, name: impl AsRef<str>) -> Option<&mut ModelExec<Model<X>>> {
        let name = name.as_ref();
        if name.is_empty() {
            return None;
        }
        self.0.iter_mut().find(|me| &me.model.name == name)
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn match_first(&self, input: &I) -> Option<&ModelExec<Model<X>>> {
        for me in &self.0 {
            if me.model.extra.enabled() && me.is_match(input) {
                return Some(me);
            }
        }
        None
    }

    fn match_all(&self, input: &I) -> Vec<&ModelExec<Model<X>>> {
        let mut rs = vec![];
        for me in &self.0 {
            if me.model.extra.enabled() && me.is_match(input) {
                rs.push(me);
            }
        }
        rs
    }
}
