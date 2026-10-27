//! 前端起不来或跑不下去的原因。

/// Wayland 前端的错误。引擎在 Server 进程里，Server 连不上不算错（键原样交给应用、过一会儿再连）。
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// libxkbcommon 加载不了：没法解读按键，不能接管键盘。
    #[error("libxkbcommon.so.0 could not be loaded")]
    Xkb,

    #[error("connect to the wayland compositor: {0}")]
    Connect(#[from] wayland_client::ConnectError),

    #[error("wayland registry: {0}")]
    Globals(#[from] wayland_client::globals::GlobalError),

    /// 合成器没有这个全局对象（KWin、GNOME 没有 input-method-v2）。
    #[error("the compositor does not offer {0}; use the fcitx edition on this desktop")]
    Missing(&'static str),

    /// 同一个 seat 上已经有别的输入法（多半是 fcitx5 的 waylandim 插件）。
    #[error("another input method already holds the seat (fcitx5's waylandim addon?)")]
    Unavailable,

    #[error("wayland dispatch: {0}")]
    Dispatch(#[from] wayland_client::DispatchError),

    #[error("wayland connection: {0}")]
    Connection(#[from] wayland_client::backend::WaylandError),
}

impl Error {
    /// 进程的退出码：2 是这个桌面 / 会话跑不了（缺协议、seat 被别的输入法占着、没有 libxkbcommon、连不上合成器），
    /// 重启也没用，systemd 单元据此不再重启（`RestartPreventExitStatus=2`）；1 是跑着跑着出的错（合成器断开、协议错误），可以重启。
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Xkb
            | Self::Connect(_)
            | Self::Globals(_)
            | Self::Missing(_)
            | Self::Unavailable => 2,
            Self::Dispatch(_) | Self::Connection(_) => 1,
        }
    }
}
