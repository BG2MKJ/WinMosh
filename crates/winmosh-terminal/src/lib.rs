#![forbid(unsafe_code)]

pub mod cell;
pub mod complete_terminal;
pub mod cursor;
pub mod diff;
pub mod framebuffer;
pub mod renderer;
pub mod rendition;
pub mod user_input;

pub fn terminal_status() -> &'static str {
    "basic VT terminal implemented"
}

pub use complete_terminal::CompleteTerminal;
pub use diff::{diff_framebuffers, CellUpdate, TerminalDiff};
pub use framebuffer::{Framebuffer, FramebufferSize};
pub use renderer::{render_diff, render_framebuffer};
pub use user_input::{UserEvent, UserInput, UserStream};
