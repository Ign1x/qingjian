//! 一个 Linux 事件的答复：多数与按键一样是共用的消息（`KeyResult`、`RequestSelection`），问右键菜单的回菜单项。

use qingjian_platform::protocol::ServerMessage;
use qingjian_platform::protocol::linux::MenuReply;

/// 一个 Linux 事件的答复。
pub(crate) enum Answer {
    /// 共用消息类型里的那几种。
    Message(ServerMessage),

    /// `LinuxEvent::Menu` 的答复，线上是 `{"Menu": …}`。
    Menu(MenuReply),
}

impl From<ServerMessage> for Answer {
    fn from(message: ServerMessage) -> Self {
        Self::Message(message)
    }
}
