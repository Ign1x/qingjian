//! 假合成器收到的、测试关心的请求，按到达顺序记下。

use super::Picture;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// 接管键盘（grab_keyboard）。
    Grab,

    /// 交还键盘（grab 的 release）。
    Release,

    /// 上屏（commit_string）。
    CommitString(String),

    /// 行内拼音（set_preedit_string），光标是 UTF-8 字节偏移。
    Preedit { text: String, begin: i32, end: i32 },

    /// 删光标前后的字（delete_surrounding_text）。
    DeleteSurrounding { before: u32, after: u32 },

    /// 上面几样一起生效（commit），带 serial。
    Commit(u32),

    /// 虚拟键盘的键位表，去掉了结尾的 0。
    Keymap(String),

    /// 经虚拟键盘转给应用的键。
    Key { code: u32, pressed: bool, time: u32 },

    /// 经虚拟键盘同步的修饰键。
    Modifiers {
        depressed: u32,
        latched: u32,
        locked: u32,
        group: u32,
    },

    /// 候选窗 surface 的一次 commit：挂着的缓冲（`None` 是收起）、viewport 的目标尺寸、buffer scale。
    Popup {
        picture: Option<Picture>,
        destination: Option<(i32, i32)>,
        scale: i32,
    },

    /// 经 cursor-shape 设的光标形状（`wp_cursor_shape_device_v1` 的 shape 值，1 是 default），带指针进来时的 serial。
    CursorShape { serial: u32, shape: u32 },

    /// 违反协议的用法：真合成器会发协议错误、断开连接。测试结束时一条都不能有。
    Violation(String),

    /// 客户端断开；因为协议错误断开的带着错误。
    Disconnected(Option<String>),
}

impl Request {
    /// input-method-v2 的文字更新：上屏、行内拼音、删前后文与让它们生效的 commit。
    pub fn is_text(&self) -> bool {
        matches!(
            self,
            Self::CommitString(_)
                | Self::Preedit { .. }
                | Self::DeleteSurrounding { .. }
                | Self::Commit(_)
        )
    }
}
