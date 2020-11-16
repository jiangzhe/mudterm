pub mod border;
pub mod cmdbar;
pub mod flow;

use crate::error::Result;
use crate::ui::buffer::Buffer;

pub use flow::*;
pub use cmdbar::*;
pub use border::*;


pub trait Widget {
    fn refresh_buffer<B: Buffer>(&mut self, buf: &mut B, cjk: bool) -> Result<()>;
}
