//! Server 对 `LinuxEvent::Menu` 的答复，线上是 `{"Menu": MenuReply}`。

use serde::{Deserialize, Serialize};

use super::MenuItem;
use crate::protocol::SessionId;

/// 右键的那一格能做的几项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MenuReply {
    /// 会话编号，由 IPC 层换成连接内的编号。
    pub session: SessionId,

    /// 按菜单顺序排好的几项；空的就不弹菜单（不在组句、在翻译评审、那一格没有候选）。
    pub items: Vec<MenuItem>,
}
