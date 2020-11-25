use crate::codec::Encoder;
use crate::runtime::engine::EngineAction;
use crate::runtime::RuntimeOutput;
use crate::ui::line::{Line, Lines, RawLine, RawLines};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// 运行时事件队列
///
/// 由于事件中包含有操作，而操作又会生成事件
/// 因此我们需要不断地遍历并处理所有事件，直到
/// 不再存在操作型事件
#[derive(Debug, Clone)]
pub struct OutputQueue(Vec<RuntimeOutput>);

impl OutputQueue {
    pub fn new() -> Self {
        Self(vec![])
    }

    pub fn send_line(&mut self, raw: RawLine, styled: Line) {
        self.send_raw_line(raw);
        self.send_styled_line(styled);
    }

    fn send_raw_line(&mut self, raw: RawLine) {
        if let Some(RuntimeOutput::ToUI(raw_lines, _)) = self.0.last_mut() {
            raw_lines.push_line(raw);
            return;
        }
        let mut raw_lines = RawLines::unbounded();
        raw_lines.push_line(raw);
        self.0.push(RuntimeOutput::ToUI(raw_lines, Lines::new()));
    }

    pub fn send_styled_line(&mut self, styled: Line) {
        if let Some(RuntimeOutput::ToUI(_, styled_lines)) = self.0.last_mut() {
            styled_lines.push_line(styled);
            return;
        }
        let mut styled_lines = Lines::new();
        styled_lines.push_line(styled);
        self.0
            .push(RuntimeOutput::ToUI(RawLines::unbounded(), styled_lines));
    }

    /// 推送命令必须以\n结尾
    pub fn send_cmd(&mut self, mut cmd: String, encoder: &Encoder) {
        // maybe directly sent from script
        if !cmd.ends_with('\n') {
            cmd.push('\n');
        }
        if let Some(RuntimeOutput::ToServer(s)) = self.0.last_mut() {
            // s.push_str(&cmd);
            if let Err(e) = encoder.encode_to(&cmd, s) {
                log::error!("encode command[{}] error {}", &cmd, e);
            }
            return;
        }
        let mut output = Vec::new();
        if let Err(e) = encoder.encode_to(&cmd, &mut output) {
            log::error!("encode command[{}] error {}", &cmd, e);
        }
        self.0.push(RuntimeOutput::ToServer(output));
    }

    pub fn drain_all(&mut self) -> Vec<RuntimeOutput> {
        self.0.drain(..).collect()
    }

    pub fn push(&mut self, ro: RuntimeOutput) {
        self.0.push(ro);
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn into_vec(self) -> Vec<RuntimeOutput> {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct ActionQueue(Arc<Mutex<VecDeque<EngineAction>>>);

impl ActionQueue {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(VecDeque::new())))
    }

    pub fn push(&self, action: EngineAction) {
        self.0.lock().unwrap().push_back(action);
    }

    pub fn len(&self) -> usize {
        self.0.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.lock().unwrap().is_empty()
    }

    pub fn drain_all(&self) -> Vec<EngineAction> {
        self.0.lock().unwrap().drain(..).collect()
    }
}
