//! 候选右键菜单（Linux 协议 v4 起）：前端发 `LinuxEvent::Menu` 问这一格能做哪几件事，
//! Server 回 `{"Menu": MenuReply}`；点中一项再发 `LinuxEvent::MenuPick`。

mod item;
mod reply;

pub use item::MenuItem;
pub use reply::MenuReply;
