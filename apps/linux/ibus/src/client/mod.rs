//! 连 Server 的那一条连接在 IBus 这边怎么共用：几个输入上下文一条连接，断了自己接回来。
//!
//! 连接本身（握手、一问一答、回包）在 `qingjian_platform::protocol::linux::client`，与 Wayland 前端共用。

mod shared;

pub use shared::Shared;
