use crate::error::{Error, Result};
use regex::Regex;
use std::collections::HashMap;
use mlua::ToLua;

/// 对原模型的抽象
pub trait Model: Sized {
    fn name(&self) -> &str;

    fn group(&self) -> &str;

    fn enabled(&self) -> bool;

    fn set_enabled(&mut self, enabled: bool);

    fn compile(self) -> Result<ModelExec<Self>>;
}

/// 抽象对正则与脚本的编译
#[derive(Debug, Clone)]
pub struct ModelExec<M> {
    pub model: M,
    re: Regex,
}

impl<M> ModelExec<M>
where
    M: Model + std::fmt::Debug,
{
    pub fn new(model: M, re: Regex) -> Self {
        Self {
            model,
            re,
        }
    }

    pub fn is_match(&self, input: impl AsRef<str>) -> bool {
        self.re.is_match(input.as_ref())
    }

    // todo: 增加style的捕获    
    pub fn captures(&self, input: impl AsRef<str>) -> Result<ModelCaptures> {
        let captures = self.re.captures(input.as_ref())
            .ok_or_else(|| Error::RuntimeError(format!("mismatch alias {:?}", self.model)))?;
        let mut names = self.re.capture_names().skip(1);
        let mut mapping = HashMap::new();
        while let Some((i, om)) = captures.iter().enumerate().skip(1).next() {
            let name = names.next().unwrap();
            if let Some(m) = om {
                if let Some(name) = name {
                    mapping.insert(NumberOrString::new_string(name), m.as_str().to_owned());
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

#[derive(Debug)]
pub struct ModelStore<T> {
    arr: Vec<T>,
}

impl<T> AsRef<[T]> for ModelStore<T> {

    fn as_ref(&self) -> &[T] {
        self.arr.as_ref()
    }
}

impl<M> ModelStore<ModelExec<M>>
where
    M: Model,
{
    pub fn new() -> Self {
        Self { arr: Vec::new() }
    }

    pub fn add(&mut self, me: ModelExec<M>) -> Result<()> {
        let idx = if me.model.name().is_empty() {
            None
        } else {
            self.get(me.model.name())
        };
        match idx {
            None => self.arr.push(me),
            Some(_) => {
                return Err(Error::RuntimeError(
                    "A trigger of that name already exists".to_owned(),
                ))
            }
        }
        Ok(())
    }

    pub fn remove(&mut self, name: impl AsRef<str>) -> Option<ModelExec<M>> {
        let name = name.as_ref();
        if name.is_empty() {
            return None;
        }
        match self.get_idx(name) {
            None => None,
            Some(idx) => Some(self.arr.swap_remove(idx)),
        }
    }

    pub fn enable(&mut self, name: impl AsRef<str>, enabled: bool) -> Option<&ModelExec<M>> {
        let name = name.as_ref();
        if name.is_empty() {
            return None;
        }
        match self.get_mut(&name) {
            None => None,
            Some(me) => {
                me.model.set_enabled(enabled);
                Some(me)
            }
        }
    }

    pub fn enable_group(&mut self, group: impl AsRef<str>, enabled: bool) -> usize {
        let group = group.as_ref();
        if group.is_empty() {
            return 0;
        }
        let mut n = 0;
        for me in self.arr.iter_mut() {
            if me.model.group() == group {
                me.model.set_enabled(enabled);
                n += 1;
            }
        }
        n
    }

    pub fn get(&self, name: impl AsRef<str>) -> Option<&ModelExec<M>> {
        let name = name.as_ref();
        if name.is_empty() {
            return None;
        }
        self.arr.iter().find(|me| me.model.name() == name)
    }

    pub fn get_mut(&mut self, name: impl AsRef<str>) -> Option<&mut ModelExec<M>> {
        let name = name.as_ref();
        if name.is_empty() {
            return None;
        }
        self.arr.iter_mut().find(|me| me.model.name() == name)
    }

    #[inline]
    fn get_idx(&self, name: impl AsRef<str>) -> Option<usize> {
        let name = name.as_ref();
        if name.is_empty() {
            return None;
        }
        self.arr.iter().position(|me| me.model.name() == name)
    }
}
