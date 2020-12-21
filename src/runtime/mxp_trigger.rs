use crate::ui::span::Span;
use crate::runtime::model::{MapModelStore, Model};
use crate::proto::Label;

// pub type MxpTriggers = MapModelStore<MxpTrigger>;

// impl MxpTriggers {
//     /// 匹配文本，返回匹配成功的触发器列表
//     ///
//     /// 与match_first不同之处在于支持多行匹配
//     pub fn trigger_first(&self, event: &Span) -> Option<&MxpTrigger> {
//         for tr in self.0.values() {
//             if let Some(matches) = tr.match_trigger(event) {
//                 return Some(matches);
//             }
//         }
//         None
//     }
// }

pub type MxpTriggerModel = Model<MxpTriggerSetting>;

pub struct MxpTriggerSetting {
    pub label: Label,
    
}