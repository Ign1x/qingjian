//! Linux 事件：释放、焦点、能力与候选回调不改 Windows 消息格式。
use super::{AppSurrounding, Capabilities, DisplayIdentity, ModeCommand};
use crate::protocol::KeyEvent;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LinuxEvent {
    Capabilities(Capabilities),

    Reset,

    Key {
        event: KeyEvent,

        release: bool,

        #[serde(default)]
        surrounding: Option<AppSurrounding>,

        #[serde(default)]
        selection_supported: bool,
    },

    Selection {
        request: u64,

        text: String,
    },

    Deactivate {
        focus_out: bool,

        client_preedit: bool,

        capability_changed: bool,
    },

    Focus {
        focused: bool,
    },

    Candidate {
        identity: DisplayIdentity,

        index: usize,
    },

    Page {
        identity: DisplayIdentity,

        next: bool,
    },

    /// 右键了当前页第 `index` 格（从 0 起）：回 `{"Menu": MenuReply}`；身份过期回 `Ignored`。
    Menu {
        identity: DisplayIdentity,

        index: usize,
    },

    /// 右键菜单里点了一项：`action` 是 [`MenuItem::id`](super::MenuItem::id)。回 `KeyResult`，
    /// 做完的那句提示在帧的 `notice` 上；身份过期回 `Ignored`。
    MenuPick {
        identity: DisplayIdentity,

        index: usize,

        action: u32,
    },

    /// 托盘 / 状态显示面上切模式：回 `KeyResult`，切了中 / 英时带 `english`。
    Indicator(ModeCommand),
}

impl LinuxEvent {
    /// 这个事件从哪一版 Linux 协议起有：会话握手报的版本低于它，Server 当协议错误断开。
    pub fn since(&self) -> u32 {
        match self {
            Self::Capabilities(_)
            | Self::Reset
            | Self::Key { .. }
            | Self::Selection { .. }
            | Self::Deactivate { .. }
            | Self::Focus { .. }
            | Self::Candidate { .. }
            | Self::Page { .. } => 3,
            Self::Menu { .. } | Self::MenuPick { .. } | Self::Indicator(_) => 4,
        }
    }
}
