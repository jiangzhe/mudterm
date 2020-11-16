pub mod block;
pub mod cmdbar;
pub mod flow;

use crate::error::Result;
use crate::ui::buffer::Buffer;

pub use block::*;
pub use cmdbar::*;
pub use flow::*;

pub trait Widget {
    fn refresh_buffer<B: Buffer>(&mut self, buf: &mut B) -> Result<()>;
}
