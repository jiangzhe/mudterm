use crate::runtime::model::{MapModelStore, Model, ModelMatch};
use bitflags::bitflags;

pub type Aliases = MapModelStore<Alias>;

pub type Alias = Model<AliasFlags>;

impl ModelMatch for Model<AliasFlags> {
    type Input = str;
    fn is_match(&self, input: &str) -> bool {
        self.re.is_match(input)
    }
}

bitflags! {
    pub struct AliasFlags: u16 {
        // const ENABLED = 0x0001;
        // todo: 实现嵌套别名
        const KEEP_EVALUATING = 0x0008;
    }
}

impl Default for AliasFlags {
    fn default() -> Self {
        Self::empty()
    }
}

impl Alias {

    pub fn keep_evaluating(&self) -> bool {
        self.extra.contains(AliasFlags::KEEP_EVALUATING)
    }

    pub fn set_keep_evaluating(&mut self, keep_evaluating: bool) {
        if keep_evaluating {
            self.extra.insert(AliasFlags::KEEP_EVALUATING);
        } else {
            self.extra.remove(AliasFlags::KEEP_EVALUATING);
        }
    }
}
