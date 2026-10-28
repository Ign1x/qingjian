//! 弹着的候选右键菜单（会话这一侧记的那一份；画什么字、悬停哪一项、摆在哪由 `crate::wayland` 记）。

/// 右键的是哪一格，菜单里各项的编号。
#[derive(Debug)]
pub struct Menu {
    /// 右键的是本页第几个候选。
    pub index: usize,

    /// 各项的编号（`MenuItem::id`），按菜单的顺序：点中第几项就带回第几个。
    actions: Vec<u32>,
}

impl Menu {
    pub fn new(index: usize, actions: Vec<u32>) -> Self {
        Self { index, actions }
    }

    /// 第 `item` 项的编号；没有这一项是 `None`。
    pub fn action(&self, item: usize) -> Option<u32> {
        self.actions.get(item).copied()
    }
}
