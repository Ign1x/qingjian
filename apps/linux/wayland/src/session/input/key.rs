//! 一次按键。

/// 键盘 grab 给的一次按下或抬起，keysym 与修饰键已经按当时的键位表算好。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    /// evdev 键码（grab 原样给的，转给虚拟键盘也用它）。
    pub code: u32,

    pub keysym: u32,

    /// X11 修饰键掩码（Shift 第 0 位、Lock 第 1 位、Control 第 2 位、Mod1–Mod5 第 3–7 位）。
    pub mask: u32,

    pub pressed: bool,

    /// 键位表说这个键按住会重复（修饰键不重复）。
    pub repeats: bool,

    /// 合成器给的时间戳（毫秒，起点不定）。
    pub time: u32,

    /// 收到这个键时的焦点代次：等 Server 回包期间输入框换了，回包就不能再落到应用里。
    pub epoch: u64,
}
