//! 会话测试的输出记录。

/// 假合成器记下的动作。
#[derive(Debug, Clone, PartialEq)]
pub enum Out {
    Grab(bool),
    Keymap,
    Forward(u32, bool),
    Modifiers(u32),
    Text(String, String),
    Show(Vec<String>),
    Hide,
    /// 弹出右键菜单，各项的字。
    Menu(Vec<String>),
    CloseMenu,
}
