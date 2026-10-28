//! 黑盒测试的架子：假合成器、假 Server、被测进程、临时目录，以及测试共用的键码与键位表。

mod compositor;
mod exit;
mod frontend;
mod log;
mod rig;
mod sandbox;
mod server;
mod targets;

pub use compositor::{BUTTON_LEFT, BUTTON_RIGHT, Compositor, DISPLAY, Picture, Request, Setup};
pub use exit::Exit;
pub use frontend::{Frontend, skip};
pub use log::{Log, PATIENCE};
pub use rig::Rig;
pub use sandbox::Sandbox;
pub use server::{Answer, MENU, Server, kind};
pub use targets::Targets;

/// 假合成器经键盘 grab 发的键位表（`tests/fixtures/keymap.xkb`）。
pub const KEYMAP: &str = include_str!("../../fixtures/keymap.xkb");

/// 测试按的键，evdev 键码（`linux/input-event-codes.h`），都在 [`KEYMAP`] 里。
pub const KEY_ESC: u32 = 1;
pub const KEY_A: u32 = 30;
pub const KEY_H: u32 = 35;
pub const KEY_N: u32 = 49;
pub const KEY_SPACE: u32 = 57;
