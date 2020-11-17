use crate::runtime::{Pattern, Scripts, Target};
use crate::error::{Error, Result};
use std::borrow::Cow;


/// 对原模型的抽象
pub trait Model: Sized {
    fn name(&self) -> &str;

    fn group(&self) -> &str;

    fn target(&self) -> Target;

    fn enabled(&self) -> bool;

    fn set_enabled(&mut self, enabled: bool);

    fn compile(self) -> Result<ModelExec<Self>>;
}

#[derive(Debug)]
/// 抽象对正则与脚本的编译
pub struct ModelExec<M> {
    pub model: M,
    pattern: Pattern,
    scripts: Scripts,
}

impl<M> ModelExec<M> 
where 
    M: Model,
{
    pub fn new(model: M, pattern: Pattern, scripts: Scripts) -> Self {
        Self{model, pattern, scripts}
    }

    pub fn is_match(&self, input: &str) -> bool {
        self.pattern.is_match(input, true)
    }

    pub fn prepare_scripts(&self, input: &str) -> Option<Cow<str>> {
        super::prepare_scripts(&self.pattern, &self.scripts, input)
    }
}


#[derive(Debug)]
pub struct ModelStore<T> {
    arr: Vec<T>,
}

impl<M> ModelStore<ModelExec<M>> 
where
    M: Model,
{

    pub fn new() -> Self {
        Self{
            arr: Vec::new(),
        }
    }

    pub fn add(&mut self, model: M) -> Result<()> {
        let me = model.compile()?;
        let idx = if me.model.name().is_empty() {
            None
        } else {
            self.get(me.model.name())
        };
        match idx {
            None => self.arr.push(me),
            Some(_) => return Err(Error::RuntimeError("A trigger of that name already exists".to_owned())),
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

    #[inline]
    fn get(&self, name: impl AsRef<str>) -> Option<&ModelExec<M>> {
        let name = name.as_ref();
        if name.is_empty() {
            return None;
        }
        self.arr.iter().find(|me| me.model.name() == name)
    }

    #[inline]
    fn get_idx(&self, name: impl AsRef<str>) -> Option<usize> {
        let name = name.as_ref();
        if name.is_empty() {
            return None;
        }
        self.arr.iter().position(|me| me.model.name() == name)
    }

    #[inline]
    fn get_mut(&mut self, name: impl AsRef<str>) -> Option<&mut ModelExec<M>> {
        let name = name.as_ref();
        if name.is_empty() {
            return None;
        }
        self.arr.iter_mut().find(|me| me.model.name() == name)
    }
}
