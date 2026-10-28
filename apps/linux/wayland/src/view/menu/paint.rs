//! 画候选右键菜单：用自己的渲染器（与候选窗共用会打断候选窗的过渡），第一次弹菜单时才建（加载字体库要几十毫秒），
//! 建不成不再试；与 macOS、Windows 一样用系统字体，深浅跟着候选窗上那一帧。

use qingjian_render::{FontLibrary, Rendered, Renderer};

use crate::view::{LOCALE, Painter};

impl Painter {
    /// 画一份菜单：`hovered` 是鼠标停在哪一项上，`scale` 与候选窗同一个倍数。画不出来记日志、返回 `None`。
    pub fn render_menu(
        &mut self,
        items: &[String],
        hovered: Option<usize>,
        scale: f32,
    ) -> Option<Rendered> {
        let dark = self.seen.dark_for(self.appearance);
        let renderer = self
            .menu_renderer
            .get_or_insert_with(|| {
                FontLibrary::system(LOCALE)
                    .map(Renderer::new)
                    .inspect_err(
                        |error| tracing::warn!(%error, "右键菜单的字体库加载失败，菜单弹不出来"),
                    )
                    .ok()
            })
            .as_mut()?;
        let items: Vec<&str> = items.iter().map(String::as_str).collect();
        renderer
            .render_menu(&items, hovered, dark, scale)
            .inspect_err(|error| tracing::warn!(%error, "右键菜单渲染失败"))
            .ok()
    }
}
