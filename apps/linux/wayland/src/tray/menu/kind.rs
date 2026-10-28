//! 菜单项的样子。

/// 一项在 dbusmenu 里画成什么。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// 一组里选一个（中文 / 英文），`true` 是选着的。
    Radio(bool),

    /// 开关（全角标点、繁体输出），`true` 是开着。
    Check(bool),

    /// 点一下做一件事（设置）。
    Plain,

    /// 灰着、点不了的一行字（版本号）。
    Note,

    Separator,
}
