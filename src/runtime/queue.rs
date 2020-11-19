use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use crate::ui::line::{RawLine, RawLines};
use crate::runtime::{RuntimeOutput, RuntimeAction};

/// 运行时时间队列
#[derive(Debug, Clone)]
pub struct OutputQueue(Arc<Mutex<VecDeque<RuntimeOutput>>>);

impl OutputQueue {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(VecDeque::new())))
    }

    pub fn push_line(&self, line: RawLine) {
        let mut evtq = self.0.lock().unwrap();
        if let Some(RuntimeOutput::ToUI(lines)) = evtq.back_mut() {
            lines.push_line(line);
            return;
        }
        let mut lines = RawLines::unbounded();
        lines.push_line(line);
        evtq.push_back(RuntimeOutput::ToUI(lines));
    }

    /// 推送命令必须以\n结尾
    pub fn push_cmd(&self, cmd: String) {
        debug_assert!(cmd.ends_with('\n'));
        let mut evtq = self.0.lock().unwrap();
        if let Some(RuntimeOutput::ToServer(s)) = evtq.back_mut() {
            s.push_str(&cmd);
            return;
        }
        evtq.push_back(RuntimeOutput::ToServer(cmd));
    }

    pub fn drain_all(&self) -> Vec<RuntimeOutput> {
        self.0.lock().unwrap().drain(..).collect()
    }

    pub fn push(&self, re: RuntimeOutput) {
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