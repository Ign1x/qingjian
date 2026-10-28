//! 会话测试的请求记录。

use qingjian_platform::protocol::linux::{Capabilities, ModeCommand};

/// 假 Server 收到的请求。
#[derive(Debug, Clone, PartialEq)]
pub enum Call {
    /// 换了应用（选着的没变不记）。
    Select(Option<String>),
    Capabilities(Capabilities),
    Focus(bool),
    Deactivate,
    Reset,
    Key {
        code: u32,

        release: bool,

        context: bool,

        selection: bool,
    },
    /// 回了选区：请求编号与选中的文字。
    Selection(u64, String),
    Poll,
    Acknowledge(u64),
    /// 托盘请求切换当前会话的模式。
    Indicator(ModeCommand),
    /// 点了第几个候选，带的展示身份的版本号。
    Candidate(usize, u64),
    /// 翻页：是不是下一页。
    Page(bool),
    /// 右键第几个候选。
    Menu(usize),
    /// 点了菜单里的一项：右键的那一格、那一项的编号。
    MenuPick(usize, u32),
}
