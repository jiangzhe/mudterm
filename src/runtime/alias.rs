use crate::runtime::model::{Model, ModelExec, ModelExtra, VecModelStore};
use bitflags::bitflags;

pub type Aliases = VecModelStore<Alias>;

pub type Alias = ModelExec<AliasModel>;

pub type AliasModel = Model<AliasFlags>;

bitflags! {
    pub struct AliasFlags: u16 {
        const ENABLED = 0x0001;
        const KEEP_EVALUATING = 0x0008;
    }
}

impl ModelExtra for AliasFlags {
    fn enabled(&self) -> bool {
        self.contains(AliasFlags::ENABLED)
    }

    fn set_enabled(&mut self, enabled: bool) {
        if enabled {
            self.insert(AliasFlags::ENABLED);
        } else {
            self.remove(AliasFlags::ENABLED);
        }
    }

    fn keep_evaluating(&self) -> bool {
        self.contains(AliasFlags::KEEP_EVALUATING)
    }

    fn set_keep_evaluating(&mut self, keep_evaluating: bool) {
        if keep_evaluating {
            self.insert(AliasFlags::KEEP_EVALUATING);
        } else {
            self.remove(AliasFlags::KEEP_EVALUATING);
        }
    }
}

impl AliasModel {

    pub fn enabled(&self) -> bool {
        self.extra.contains(AliasFlags::ENABLED)
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        if enabled {
            self.extra.insert(AliasFlags::ENABLED);
        } else {
            self.extra.remove(AliasFlags::ENABLED);
        }
    }

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
