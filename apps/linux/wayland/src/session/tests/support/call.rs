//! 会话测试的请求记录。

use qingjian_platform::protocol::linux::Capabilities;

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
}
