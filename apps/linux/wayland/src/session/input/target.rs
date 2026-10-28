//! 托盘上点出来的切换：某一项要切到哪边。

use qingjian_platform::protocol::linux::{InputMode, ModeCommand};

/// 托盘要的模式。按目标而不是「翻转」：托盘上画着的模式可能比 Server 的旧（还没收到回包、活动窗口换了应用），
/// 照着翻转会翻反；会话先问一遍那个会话现在的模式，不是目标才发切换。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeTarget {
    /// 中 / 英：`true` 是英文。
    English(bool),

    /// 当前中 / 英模式那一份全角标点：`true` 是全角。
    FullWidth(bool),

    /// 繁体输出：`true` 是繁体。
    Traditional(bool),
}

impl ModeTarget {
    /// 从 `current` 到这个目标要发的切换命令；已经是了为 `None`。不知道现在的模式（老 Server）时照发。
    pub fn command(self, current: Option<&InputMode>) -> Option<ModeCommand> {
        let (wanted, now, command) = match self {
            Self::English(on) => (
                on,
                current.map(|mode| mode.english),
                ModeCommand::ToggleEnglish,
            ),
            Self::FullWidth(on) => (
                on,
                current.map(|mode| mode.full_width),
                ModeCommand::TogglePunctuation,
            ),
            Self::Traditional(on) => (
                on,
                current.map(|mode| mode.traditional),
                ModeCommand::ToggleTraditional,
            ),
        };
        (now != Some(wanted)).then_some(command)
    }
}
