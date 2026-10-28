//! 黑盒测试的架子：假合成器、假 Server、被测进程、临时目录，以及测试共用的键码与键位表。

mod compositor;
mod exit;
mod frontend;
mod log;
mod rig;
mod sandbox;
mod server;

pub use compositor::{Compositor, DISPLAY, Field, Picture, Request, Setup};
pub use exit::Exit;
pub use frontend::{Frontend, skip};
pub use log::{Log, PATIENCE};
pub use rig::Rig;
pub use sandbox::Sandbox;
pub use server::{Answer, Server, kind};

/// 假合成器经键盘 grab 发的键位表（`tests/fixtures/keymap.xkb`）。
pub const KEYMAP: &str = include_str!("../../fixtures/keymap.xkb");

/// 测试按的键，evdev 键码（`linux/input-event-codes.h`），都在 [`KEYMAP`] 里。
pub const KEY_A: u32 = 30;
pub const KEY_N: u32 = 49;
pub const KEY_SPACE: u32 = 57;
pub const KEY_T: u32 = 20;
pub const KEY_LEFTSHIFT: u32 = 42;
pub const KEY_LEFTALT: u32 = 56;

/// 修饰键在 xkb 状态里的位（grab 的 modifiers 事件用它）：Shift 是第 0 位，Alt 是 Mod1、第 3 位。
pub const SHIFT: u32 = 1;
pub const ALT: u32 = 1 << 3;

/// 只留 input-method-v2 的文字更新（上屏、行内拼音、删前后文与让它们生效的 commit）。
pub fn text_updates(requests: &[Request]) -> Vec<Request> {
    requests
        .iter()
        .filter(|request| request.is_text())
        .cloned()
        .collect()
}
