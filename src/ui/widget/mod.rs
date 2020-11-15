pub mod border;
pub mod cmdbar;

use crate::error::Result;
use crate::ui::buffer::Buffer;

pub trait Widget {
    fn refresh_buffer<B: Buffer>(&mut self, buf: &mut B, cjk: bool) -> Result<()>;
}
