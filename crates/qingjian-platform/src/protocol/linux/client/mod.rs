//! 连 Server 的客户端：一条 Unix socket，一问一答。IBus 前端与 Wayland 前端共用。
//!
//! 与 Fcitx5 插件（C++，`apps/linux/fcitx5/src/ipc/`）是同一个协议的两份实现，
//! 线上类型就在父模块，两边都从那里取。

mod connection;
mod error;
mod reply;

#[cfg(test)]
mod tests;

pub use connection::Connection;
pub use error::ClientError;
pub use reply::Reply;
