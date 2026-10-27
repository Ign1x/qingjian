//! X11 keysym：常量与「这个 keysym 敲出来是哪个字符」。
//!
//! IBus 的 `ProcessKeyEvent` 直接给 keysym，Wayland 前端用 xkbcommon 按键码算出 keysym。
//! 取值来自 X11 的 `keysymdef.h`，是三十年没动过的公开常量，不值得为它引一个依赖。
//!
//! 这里只列 [`super::to_key_event`] 用得到的那些：功能键区、小键盘、Shift 与 Ctrl。

/// 功能键区（`0xff00` 起）。
pub const BACKSPACE: u32 = 0xff08;
pub const TAB: u32 = 0xff09;
pub const RETURN: u32 = 0xff0d;
pub const ESCAPE: u32 = 0xff1b;
pub const HOME: u32 = 0xff50;
pub const LEFT: u32 = 0xff51;
pub const UP: u32 = 0xff52;
pub const RIGHT: u32 = 0xff53;
pub const DOWN: u32 = 0xff54;
pub const PAGE_UP: u32 = 0xff55;
pub const PAGE_DOWN: u32 = 0xff56;
pub const END: u32 = 0xff57;
pub const DELETE: u32 = 0xffff;

/// Shift + Tab：X11 不是「Tab 加 Shift 修饰键」，而是另一个 keysym。
pub const ISO_LEFT_TAB: u32 = 0xfe20;

/// 两个 Shift 键本身：中 / 英切换要认它的按下与抬起（`[shortcut] switch_mode`）。
pub const SHIFT_L: u32 = 0xffe1;
pub const SHIFT_R: u32 = 0xffe2;

/// 小键盘：NumLock 灭着时方向 / 编辑键走这一段，亮着时 `KP_0`–`KP_9` 出数字。
pub const KP_ENTER: u32 = 0xff8d;
pub const KP_HOME: u32 = 0xff95;
pub const KP_LEFT: u32 = 0xff96;
pub const KP_UP: u32 = 0xff97;
pub const KP_RIGHT: u32 = 0xff98;
pub const KP_DOWN: u32 = 0xff99;
pub const KP_PAGE_UP: u32 = 0xff9a;
pub const KP_PAGE_DOWN: u32 = 0xff9b;
pub const KP_END: u32 = 0xff9c;
pub const KP_BEGIN: u32 = 0xff9d;
pub const KP_INSERT: u32 = 0xff9e;
pub const KP_DELETE: u32 = 0xff9f;

/// 小键盘的运算符键。
pub const KP_MULTIPLY: u32 = 0xffaa;
pub const KP_ADD: u32 = 0xffab;
pub const KP_SEPARATOR: u32 = 0xffac;
pub const KP_SUBTRACT: u32 = 0xffad;
pub const KP_DECIMAL: u32 = 0xffae;
pub const KP_DIVIDE: u32 = 0xffaf;

/// 小键盘数字段。
pub const KP_0: u32 = 0xffb0;
pub const KP_9: u32 = 0xffb9;

/// Control 键本身：中 / 英切换可以配成单击 Ctrl（`[shortcut] switch_mode`）。
pub const CONTROL_L: u32 = 0xffe3;
pub const CONTROL_R: u32 = 0xffe4;

/// 这个 keysym 敲出来是哪个可见字符；功能键与修饰键没有字符，是 `None`。
///
/// 与 Fcitx5 插件一样按 keysym 查 Unicode（`Key::keySymToUnicode`，与 xkbcommon 同一张表，含小键盘的运算符与
/// 各旧字符集段），再丢掉 0x20 以下与 0x7f——Server 那边 `KeyEvent::character` 只认可见字符，功能键靠 `virtual_key`。
pub fn to_char(keysym: u32) -> Option<char> {
    xkeysym::Keysym::new(keysym)
        .key_char()
        .filter(|c| *c >= ' ' && *c != '\x7f')
}
