use crate::proto::Element;
use crate::runtime::model::{MapModelStore, Model, ModelMatch};
use crate::runtime::trigger::TriggerFlags;

pub type MxpTriggers = MapModelStore<MxpTrigger>;

impl MxpTriggers {
    pub fn trigger_first(&self, input: &Element) -> Option<&MxpTrigger> {
        self.0.values()
            .find(|tr| tr.is_match(input))
    }

    pub fn trigger_all(&self, input: &Element) -> Vec<&MxpTrigger> {
        self.0.values()
            .filter(|tr| tr.is_match(input))
            .collect()
    }
}

pub type MxpTrigger = Model<MxpTriggerExtra>;

#[derive(Debug, Clone, PartialEq)]
pub struct MxpTriggerExtra {
    pub label: String,
    pub flags: TriggerFlags,
}

impl Default for MxpTriggerExtra {
    fn default() -> Self {
        Self{label: String::new(), flags: TriggerFlags::empty()}
    }
}

impl MxpTriggerExtra {
    pub fn one_shot(&self) -> bool {
        self.flags.contains(TriggerFlags::ONESHOT)
    }

    pub fn set_one_shot(&mut self, one_shot: bool) {
        if one_shot {
            self.flags.insert(TriggerFlags::ONESHOT);
        } else {
            self.flags.remove(TriggerFlags::ONESHOT);
        }
    }
}

impl ModelMatch for MxpTrigger {
    type Input = Element;

    fn is_match(&self, input: &Self::Input) -> bool {
        if let Element::Span(span) = input {
            return span.label.ty() == &self.extra.label &&
                self.re.is_match(&span.content);
        }
        input.ty() == &self.extra.label
    }
}