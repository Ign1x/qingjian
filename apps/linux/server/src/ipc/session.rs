//! 一条连接内的会话映射与独立协商状态。
use qingjian_platform::protocol::SessionId;
use qingjian_platform::protocol::linux::DisplayIdentity;

pub(super) struct Session {
    pub(super) global: SessionId,

    pub(super) identity: Option<DisplayIdentity>,

    /// `LinuxHello` 握手的 Linux 协议版本，没握手为 0；这个会话只能发这一版里有的事件。
    pub(super) version: u32,

    pub(super) capabilities: bool,
}
