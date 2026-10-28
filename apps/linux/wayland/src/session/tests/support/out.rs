//! 会话测试的输出记录。

use qingjian_platform::protocol::linux::{InputMode, KeyHintView};

/// 假合成器记下的动作。
#[derive(Debug, Clone, PartialEq)]
pub enum Out {
    Grab(bool),
    Keymap,
    Forward(u32, bool),
    Modifiers(u32),
    Text(String, String),
    Show(Vec<String>),
    /// 候选窗旁的键位提示，`None` 收起键盘。
    KeyHint(Option<KeyHintView>),
    Hide,
    /// 模式交给显示面，`None` 是不知道了。
    Mode(Option<InputMode>),
    /// 在候选窗的位置亮中 / 英提示。
    Hint(InputMode),
    /// 弹出右键菜单，各项的字。
    Menu(Vec<String>),
    CloseMenu,
}
