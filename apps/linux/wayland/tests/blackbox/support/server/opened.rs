//! 假 Server 的一条连接上开着的一个会话。

use qingjian_platform::protocol::linux::Capabilities;

/// 握手之前上下文是 `None`，报能力之前 `capabilities` 是 `None`（这时发别的事件真 Server 断开连接）。
pub struct Opened {
    pub context: Option<String>,

    pub capabilities: Option<Capabilities>,
}
