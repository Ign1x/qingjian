//! 一个会话现在的输入模式：Linux Server 附在每个 `KeyResult` / `Update` 回包上的 `mode`，
//! 前端拿它画状态（托盘、中 / 英提示、候选窗主题里的 `mode.*`）。

use serde::{Deserialize, Serialize};

use crate::Scheme;

/// 当前输入模式。字段与渲染器的 `Mode` 一一对应，方案名按状态条的写法给。
///
/// 回包上只增不改的字段，不升协议版本：Fcitx5 / Fcitx4 插件不读它，老 Server 不带它。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct InputMode {
    /// 英文模式（`false` 为中文）。
    pub english: bool,

    /// 繁体输出开着（`[general] traditional_variant` 不是 `off`）。
    pub traditional: bool,

    /// 当前中 / 英模式那一份全角标点开着（中英各记一份配置）。
    pub full_width: bool,

    /// 状态条上的方案名（[`crate::scheme_label`]）：单开全拼时是空串，状态条只写「中」。
    pub scheme: String,
}

impl InputMode {
    /// 主题显示的方案名（[`crate::scheme_name`]）：同 [`Self::scheme`]，只是单开全拼时也写出「全拼」。
    pub fn scheme_name(&self) -> &str {
        if self.scheme.is_empty() {
            Scheme::Pinyin.label()
        } else {
            &self.scheme
        }
    }
}
