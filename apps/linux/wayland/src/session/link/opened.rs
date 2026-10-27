//! 连接上开着的一个会话。

use qingjian_platform::protocol::SessionId;
use qingjian_platform::protocol::linux::Capabilities;

#[derive(Debug)]
pub struct Opened {
    /// 哪个应用的；`None` 是不知道哪个应用时共用的那一个。
    pub app: Option<String>,

    /// 报给 Server 的上下文标识（`LinuxHello.context`）。
    pub context: String,

    pub id: SessionId,

    /// 最近报给它的能力：一样就不再报（Server 收到不同的能力会丢掉组句）。
    pub capabilities: Capabilities,

    /// 最近一次用到它时的计数：要关会话时关数最小的。
    pub used: u64,
}
