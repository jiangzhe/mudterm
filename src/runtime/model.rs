use crate::error::{Error, Result};
use regex::Regex;
use std::collections::HashMap;
use mlua::ToLua;

#[derive(Debug, Clone, PartialEq)]
pub struct Model<X> {
    pub name: String,
    pub group: String,
    pub pattern: String,
    pub extra: X,
}

impl<X: std::fmt::Debug> Model<X> {

    pub fn compile(self) -> Result<ModelExec<Self>> {
        let re = Regex::new(&self.pattern)?;
        Ok(ModelExec::new(self, re))
    }
}

pub trait ModelExtra {

    fn enabled(&self) -> bool;

    fn set_enabled(&mut self, enabled: bool);

    fn keep_evaluating(&self) -> bool;

    fn set_keep_evaluating(&mut self, keep_evaluating: bool);
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

impl<X: std::fmt::Debug> ModelExec<Model<X>> {
    pub fn new(model: Model<X>, re: Regex) -> Self {
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
pub struct MapModelStore<M>(HashMap<String, M>);


#[derive(Debug)]
pub struct VecModelStore<ME> {
    arr: Vec<ME>,
}

impl<X: ModelExtra> AsRef<[ModelExec<Model<X>>]> for VecModelStore<ModelExec<Model<X>>> {

    fn as_ref(&self) -> &[ModelExec<Model<X>>] {
        self.arr.as_ref()
    }
}

impl<X: ModelExtra> VecModelStore<ModelExec<Model<X>>> {
    pub fn new() -> Self {
        Self { arr: Vec::new() }
    }

    pub fn add(&mut self, me: ModelExec<Model<X>>) -> Result<()> {
        let opt = if me.model.name.is_empty() {
            None
        } else {
            self.get(&me.model.name)
        };
        match opt {
            None => self.arr.push(me),
            Some(_) => {
                return Err(Error::RuntimeError(
                    "A trigger of that name already exists".to_owned(),
                ))
            }
        }
        Ok(())
    }

    pub fn remove(&mut self, name: impl AsRef<str>) -> Option<ModelExec<Model<X>>> {
        let name = name.as_ref();
        if name.is_empty() {
            return None;
        }
        match self.get_idx(name) {
            None => None,
            Some(idx) => Some(self.arr.swap_remove(idx)),
        }
    }

    pub fn enable(&mut self, name: impl AsRef<str>, enabled: bool) -> Option<&ModelExec<Model<X>>> {
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

    pub fn enable_group(&mut self, group: impl AsRef<str>, enabled: bool) -> usize {
        let group = group.as_ref();
        if group.is_empty() {
            return 0;
        }
        let mut n = 0;
        for me in self.arr.iter_mut() {
            if &me.model.group == group {
                me.model.extra.set_enabled(enabled);
                n += 1;
            }
        }
        n
    }

    pub fn get(&self, name: impl AsRef<str>) -> Option<&ModelExec<Model<X>>> {
        let name = name.as_ref();
        if name.is_empty() {
            return None;
        }
        self.arr.iter().find(|me| &me.model.name == name)
    }

    pub fn get_mut(&mut self, name: impl AsRef<str>) -> Option<&mut ModelExec<Model<X>>> {
        let name = name.as_ref();
        if name.is_empty() {
            return None;
        }
        self.arr.iter_mut().find(|me| &me.model.name == name)
    }

    #[inline]
    fn get_idx(&self, name: impl AsRef<str>) -> Option<usize> {
        let name = name.as_ref();
        if name.is_empty() {
            return None;
        }
        self.arr.iter().position(|me| &me.model.name == name)
    }
}
