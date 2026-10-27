//! IBus 自己的按键约定。keysym + 修饰键掩码 → `KeyEvent` 的表是与 Wayland 前端共用的那份，
//! 在 `qingjian_platform::protocol::linux::key`。

/// 抬键事件的标志位：IBus 把按下与抬起走同一个回调，靠它区分。
const RELEASE: u32 = 1 << 30;

/// 这是一次抬键（不是按下）。
///
/// 抬键**照样要报给 Server**：中 / 英切换认的是 Shift 的抬起。这里只负责说清是哪一种。
pub fn is_release(state: u32) -> bool {
    state & RELEASE != 0
}
