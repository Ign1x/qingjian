//! 鼠标在候选窗上的一次按下。

use super::Spot;

/// 左键或右键在候选窗上按下了；按在什么上已经按画出来的那一张认好。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Click {
    /// 右键（否则是左键）。
    pub right: bool,

    /// 按在什么上；候选窗上没有可点的东西的地方（背景、拼音行、页码）是 `None`。
    pub spot: Option<Spot>,

    /// 收到这一下时的焦点代次，同 [`super::Key::epoch`]。
    pub epoch: u64,
}
