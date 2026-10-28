//! 弹着的候选右键菜单（画的那一侧）：各项的字、悬停项、摆在候选窗的哪儿，以及按悬停项画好的位图。
//! 怎么画见 `paint.rs`（[`super::Painter::render_menu`]），怎么和候选窗合成见 [`super::Canvas`]。

mod paint;

use qingjian_render::Rendered;

pub struct Menu {
    items: Vec<String>,

    /// 鼠标停在哪一项上。
    pub hovered: Option<usize>,

    /// 菜单卡片左上角在候选窗内容区里的位置（点）。
    pub anchor: (f32, f32),

    /// 按 `hovered` 画好的位图。
    pub rendered: Rendered,
}

impl Menu {
    /// 刚弹出的菜单：没有悬停项。
    pub fn new(items: Vec<String>, anchor: (f32, f32), rendered: Rendered) -> Self {
        Self {
            items,
            hovered: None,
            anchor,
            rendered,
        }
    }

    pub fn items(&self) -> &[String] {
        &self.items
    }
}
