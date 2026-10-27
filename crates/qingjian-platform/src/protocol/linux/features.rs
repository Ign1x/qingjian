//! 前端在 `LinuxHello` 里报的显示能力：自己画候选窗的前端（Wayland 原生版）能画横排矩阵与键位提示，
//! 借框架面板显示的（Fcitx5 / Fcitx4 插件、IBus 前端）画不了，Server 按它决定这两样开不开。
use serde::{Deserialize, Serialize};

/// 线上是 `LinuxHello` 的 `features` 字段，只增不改：没报（C++ 插件）或少报的项按画不了算。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Features {
    /// 横排时 ↑ / ↓ 把候选展开成多行（`[general] horizontal_grid`）：帧上的 `columns` / `column_ems` 照着排。
    /// 画不了的前端那里 ↑ / ↓ 照旧只移高亮。
    pub grid: bool,

    /// 键位提示（`[general] key_hint`）：双拼键位或五笔字根图，画在候选窗旁。
    pub key_hint: bool,
}
