use crate::runtime::{RuntimeAction, RuntimeEvent, RuntimeOutput};
use crate::ui::line::{Line, Lines, RawLine, RawLines};
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

    pub fn push_line(&self, raw: RawLine, styled: Line) {
        self.push_raw_line(raw);
        self.push_styled_line(styled);
    }

    fn push_raw_line(&self, raw: RawLine) {
        let mut evtq = self.0.lock().unwrap();
        if let Some(RuntimeEvent::Output(RuntimeOutput::ToUI(raw_lines, _))) = evtq.back_mut() {
            raw_lines.push_line(raw);
            return;
        }
        let mut raw_lines = RawLines::unbounded();
        raw_lines.push_line(raw);
        evtq.push_back(RuntimeEvent::Output(RuntimeOutput::ToUI(
            raw_lines,
            Lines::new(),
        )));
    }

    pub fn push_styled_line(&self, styled: Line) {
        let mut evtq = self.0.lock().unwrap();
        if let Some(RuntimeEvent::Output(RuntimeOutput::ToUI(_, styled_lines))) = evtq.back_mut() {
            styled_lines.push_line(styled);
            return;
        }
        let mut styled_lines = Lines::new();
        styled_lines.push_line(styled);
        evtq.push_back(RuntimeEvent::Output(RuntimeOutput::ToUI(
            RawLines::unbounded(),
            styled_lines,
        )));
    }

    /// 推送命令必须以\n结尾
    pub fn push_cmd(&self, mut cmd: String) {
        // maybe directly sent from script
        if !cmd.ends_with('\n') {
            cmd.push('\n');
        }
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
