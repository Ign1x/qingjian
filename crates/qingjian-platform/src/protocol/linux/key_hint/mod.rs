//! 键位提示（`[general] key_hint`）：Linux Server 附在 `KeyResult` / `Update` 上的 `key_hint`，自绘候选窗的前端拿它在候选窗旁画键盘。
//! 只发给握手报了 [`Features::key_hint`](super::Features) 的会话，不该显示时没有这个字段；什么时候显示由 Server 定（同 Windows Server）。
//! 换成渲染器的键盘在 `display` 模块（`render` 特性）。

mod chart;
mod content;
mod view;

#[cfg(test)]
mod tests;

pub use chart::RootChartId;
pub use content::KeyHintContent;
pub use view::KeyHintView;
