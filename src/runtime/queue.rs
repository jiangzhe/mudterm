use crate::runtime::{RuntimeAction, RuntimeEvent, RuntimeOutput};
use crate::ui::line::{RawLine, RawLines};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// 运行时事件队列
///
/// 由于事件中包含有操作，而操作又会生成事件
/// 因此我们需要不断地遍历并处理所有事件，直到
/// 不再存在操作型事件
#[derive(Debug, Clone)]
pub struct OutputQueue(Arc<Mutex<VecDeque<RuntimeEvent>>>);

impl OutputQueue {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(VecDeque::new())))
    }

    pub fn push_line(&self, line: RawLine) {
        let mut evtq = self.0.lock().unwrap();
        if let Some(RuntimeEvent::Output(RuntimeOutput::ToUI(lines))) = evtq.back_mut() {
            lines.push_line(line);
            return;
        }
        let mut lines = RawLines::unbounded();
        lines.push_line(line);
        evtq.push_back(RuntimeEvent::Output(RuntimeOutput::ToUI(lines)));
    }

    /// 推送命令必须以\n结尾
    pub fn push_cmd(&self, cmd: String) {
        debug_assert!(cmd.ends_with('\n'));
        let mut evtq = self.0.lock().unwrap();
        if let Some(RuntimeEvent::Output(RuntimeOutput::ToServer(s))) = evtq.back_mut() {
            s.push_str(&cmd);
            return;
        }
        evtq.push_back(RuntimeEvent::Output(RuntimeOutput::ToServer(cmd)));
    }

    pub fn drain_all(&self) -> Vec<RuntimeEvent> {
        self.0.lock().unwrap().drain(..).collect()
    }

    pub fn push(&self, re: RuntimeEvent) {
        self.0.lock().unwrap().push_back(re);
    }

    pub fn len(&self) -> usize {
        self.0.lock().unwrap().len()
    }
}

pub struct ActionQueue(VecDeque<RuntimeAction>);

impl ActionQueue {
    pub fn new() -> Self {
        Self(VecDeque::new())
    }

    pub fn push(&mut self, action: RuntimeAction) {
        self.0.push_back(action);
    }
}
