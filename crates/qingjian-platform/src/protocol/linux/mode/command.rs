//! 托盘 / 状态显示面上交给 Server 的模式切换（`LinuxEvent::Indicator`，Linux 协议 v4 起）。

use serde::{Deserialize, Serialize};

/// 切换模式的那几项。
///
/// 不复用 Windows 的 [`crate::protocol::IndicatorCommand`]：那边中 / 英由 DLL 自己切、也没有简繁，
/// 给它加变体又得升 Windows 那套协议版本（Fcitx5 / Fcitx4 插件写死了那个数）。Linux 的模式全在 Server。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModeCommand {
    /// 切换中 / 英，与单击 Shift 一样：组句中的拼音原样上屏；`[general] english_mode` 关着时不切。
    ToggleEnglish,

    /// 翻转当前中 / 英模式那一份全角标点，写回配置。
    TogglePunctuation,

    /// 在简体与上次用的繁体字形之间切换输出，写回配置；组句中的候选当场换字形。
    ToggleTraditional,
}
