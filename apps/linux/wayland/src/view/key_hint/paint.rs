//! 画键位提示的键盘：与候选窗一样用自己的渲染器（共用会打断候选窗的过渡动画），第一次用才建，加载字体库要几十毫秒；
//! 建不成为 `Some(None)`，之后不再试。字根图的字形解析一次留着复用，双拼的键随敲的音节变、每次现算。
//! 深浅与候选窗一致，整块再乘回包上的 `opacity`（预乘 alpha 一起乘）。

use qingjian_platform::display::key_hint_keyboard;
use qingjian_platform::protocol::linux::{KeyHintContent, KeyHintView};
use qingjian_render::{FontLibrary, Rendered, Renderer, system_fonts};

use super::super::bitmap::fade;
use super::super::{LOCALE, Painter};

impl Painter {
    /// 画出回包带的键位提示键盘，`scale` 与候选窗同一个倍数。画不出来（字体库没建起来）记日志、返回 `None`。
    pub fn render_key_hint(&mut self, view: &KeyHintView, scale: f32) -> Option<Rendered> {
        let dark = self.seen.dark_for(self.appearance);
        let theme = self.theme.with_dark(dark);
        let keyboard = match &view.content {
            // 字根图的字形解析要遍历整张表，同一张图留着上次的
            KeyHintContent::Roots(_) => self
                .roots
                .get_or_insert_with(|| key_hint_keyboard(&view.content))
                .clone(),
            // 双拼键位标着敲了哪些键之后能接成哪些音节，每次都得现换
            KeyHintContent::Shuangpin(_) => key_hint_keyboard(&view.content),
        };
        let renderer = self
            .key_hint_renderer
            .get_or_insert_with(|| {
                FontLibrary::system(LOCALE)
                    .map(Renderer::new)
                    .inspect_err(
                        |error| tracing::warn!(%error, "键位提示的字体库加载失败，键盘画不出来"),
                    )
                    .ok()
                    .map(|mut renderer| {
                        renderer.load_theme_fonts(&theme, system_fonts::family_files);
                        renderer
                    })
            })
            .as_mut()?;
        let mut rendered = renderer
            .render_keyboard(&keyboard, &theme, scale, view.zoom())
            .inspect_err(|error| tracing::warn!(%error, "键位提示渲染失败"))
            .ok()?;
        fade(rendered.pixmap.data_mut(), view.alpha());
        Some(rendered)
    }
}
