//! 候选右键菜单里的一项。

use serde::{Deserialize, Serialize};

/// 菜单里的一项：前端只管照着画、点中时把 `id` 原样带回 `LinuxEvent::MenuPick`，做什么由 Server 定。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MenuItem {
    /// 稳定编号（[`crate::candidate_menu::CandidateAction::id`]）。
    pub id: u32,

    /// 菜单上显示的字（「置顶」「降权」「删除」……）。
    pub label: String,
}
