//! 候选窗上能点的东西。

/// 鼠标落在候选窗的什么上。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spot {
    /// 本页第几个候选（从 0 起）；候选右侧的译词也算它：Linux 协议只有点候选，没有点哪条译词。
    Candidate(usize),

    /// 右键菜单的第几项（从 0 起）。
    MenuItem(usize),

    /// 右键菜单上，但不在哪一项上（菜单的边）。
    Menu,
}
