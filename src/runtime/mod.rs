pub mod alias;
pub mod cache;
pub mod model;
pub mod queue;
pub mod sub;
pub mod timer;
pub mod trigger;
pub mod engine;
pub mod vars;
pub mod init;
pub mod delay_queue;

use crate::error::Result;
use crate::event::NextStep;
use crate::ui::line::{Lines, RawLines};

pub use engine::{Engine, EngineAction};

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeOutput {
    /// 发送给服务器的命令
    ToServer(Vec<u8>),
    /// 发送给UI的文本（包含原始文本，以及格式解析后的文本）
    ToUI(RawLines, Lines),
}

/// 运行时事件回调
pub trait RuntimeOutputHandler {
    fn on_runtime_output(&mut self, output: RuntimeOutput) -> Result<NextStep>;
}
