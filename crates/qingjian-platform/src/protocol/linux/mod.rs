//! Linux 独立的面板协议扩展；Windows 的 `ClientMessage` / `ServerMessage` 与版本号不变。
//!
//! 与 Windows 那套同样的道理（见父模块文档）：Server 进程扛 [`qingjian_core::Engine`]，
//! 各框架的前端（Fcitx5 addon、IBus 前端、Wayland 前端）只做事件翻译与贴图，所以线上类型必须放在
//! **两端都能依赖、又不拖进 Engine 依赖树**的这里。两个 Rust 前端共用的客户端（[`client`](crate::protocol::linux::client)）与键表（[`key`](crate::protocol::linux::key)）
//! 也是两端的约定，一并放这里。

mod capabilities;
#[cfg(target_os = "linux")]
pub mod client;
mod display;
mod event;
mod features;
mod identity;
#[cfg(target_os = "linux")]
pub mod key;
mod key_hint;
mod menu;
mod mode;
mod request;
#[cfg(target_os = "linux")]
mod socket;
mod surrounding;

pub use capabilities::Capabilities;
pub use display::DisplayAcknowledged;
pub use event::LinuxEvent;
pub use features::Features;
pub use identity::DisplayIdentity;
pub use key_hint::{KeyHintContent, KeyHintView, RootChartId};
pub use menu::{MenuItem, MenuReply};
pub use mode::{InputMode, ModeCommand};
pub use request::LinuxRequest;
#[cfg(target_os = "linux")]
pub use socket::socket_path;
pub use surrounding::AppSurrounding;

/// Linux 面板扩展的版本，前端在 `LinuxHello` 里带上。
/// 与 [`super::PROTOCOL_VERSION`] 各自独立：那个管 Windows 那套消息，这个管 Linux 这几种事件。
///
/// 兼容规则：
/// - 前端要发新的 [`LinuxEvent`] 变体就 +1，变体从哪一版起有记在 [`LinuxEvent::since`]。
///   Server 接受 [`LINUX_UI_PROTOCOL_MIN`] 到本版之间的 `LinuxHello`，回包回显前端报的版本；
///   一个会话只能发它握手那一版里有的事件，多发的当协议错误断开。
/// - 回包上只增不改的字段（`identity`、`mode`、`key_hint`、帧上的 `translation_pending`、释义的 `annotation`）不升版：
///   前端按原始 JSON 取，没有就当缺省。只回给新事件的新消息（`Menu`）也不影响老前端。
/// - `LinuxHello` 里的显示能力（[`Features`]）同样只增不改、不升版：没报的按画不了算，Server 照旧。
/// - `OpenSession` 回包里的 `linux_ui.version` 固定报 [`LINUX_UI_PROTOCOL_MIN`]：那时还没握手，
///   Server 不知道前端是哪一版，而 Fcitx5 / Fcitx4 插件要它等于 3。
pub const LINUX_UI_PROTOCOL: u32 = 4;

/// Server 仍接受的最老 `LinuxHello` 版本。Fcitx5 / Fcitx4 插件（C++）写死 3，不再支持它们之前不能提高。
pub const LINUX_UI_PROTOCOL_MIN: u32 = 3;
