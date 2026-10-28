//! 点了菜单项做什么。

use crate::session::ModeTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// 切模式：排给主线程，由会话发给 Server。
    Mode(ModeTarget),

    /// 拉起设置程序：托盘线程自己做，不经过会话。
    Settings,
}
