//! 一次回包上的键位提示：画哪一种键盘，连同窗口的不透明度与键盘大小。

use serde::{Deserialize, Serialize};

use super::KeyHintContent;

/// 线上是回包上的 `key_hint`：`{"content": …, "opacity": 80, "size": 100}`。
/// 回包上只增不改的字段，不升协议版本；Fcitx5 / Fcitx4 插件与 IBus 前端不报 `features.key_hint`，收不到它。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyHintView {
    pub content: KeyHintContent,

    /// 窗口不透明度，百分比（`[general] key_hint_opacity`，Server 已夹到 30–100）。
    pub opacity: u8,

    /// 键盘大小，百分比（`[general] key_hint_size`，Server 已夹到 60–150，100 为原大）。
    pub size: u8,
}

impl KeyHintView {
    /// 整块的不透明度（0–1），乘到贴出去的位图上。
    pub fn alpha(&self) -> f32 {
        f32::from(self.opacity.min(100)) / 100.0
    }

    /// 键盘的缩放倍数（1 为原大），即 `Renderer::render_keyboard` 的 `zoom`。
    pub fn zoom(&self) -> f32 {
        f32::from(self.size) / 100.0
    }
}
