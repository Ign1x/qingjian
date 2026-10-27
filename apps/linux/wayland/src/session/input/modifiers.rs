//! 修饰键状态。

/// grab 给的修饰键与布局组，原样转给虚拟键盘。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub depressed: u32,

    pub latched: u32,

    pub locked: u32,

    pub group: u32,
}
