//! 中 / 英提示：切换的那一下在候选窗的位置亮一格状态条，用主题的 `status` 分节画（与 Windows 的悬浮状态条同一个画法）。

use qingjian_platform::display;
use qingjian_platform::protocol::linux::InputMode;
use qingjian_render::{Rendered, StatusCell};

use super::Painter;

impl Painter {
    /// 画中 / 英提示：一格（[`status_text`]），按 `scale` 倍出位图；出错记日志返回 `None`。
    /// 状态条不跟设置里的候选字号（渲染器自己去掉了），深浅跟上一帧候选窗的外观设置走。
    pub fn render_status(&mut self, mode: &InputMode, scale: f32) -> Option<Rendered> {
        let dark = self.desktop.preference().dark_for(self.appearance);
        let cells = [StatusCell::text(status_text(mode), true)];
        self.renderer
            .render_status(
                &cells,
                &display::render_mode(mode),
                &self.theme.with_dark(dark),
                scale,
            )
            .inspect_err(|error| tracing::warn!(%error, "中 / 英提示渲染失败"))
            .ok()
            .map(|status| status.rendered)
    }
}

/// 提示那一格的字：英文是「英」，中文是「中」，开着双拼、注音或形码时跟方案名（同 Windows 状态条的模式格）。
pub fn status_text(mode: &InputMode) -> String {
    if mode.english {
        "英".to_owned()
    } else if mode.scheme.is_empty() {
        "中".to_owned()
    } else {
        format!("中 · {}", mode.scheme)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use qingjian_platform::protocol::linux::InputMode;

    use super::super::Painter;
    use super::status_text;
    use crate::portal::Desktop;

    /// 英文只写「英」；中文单开全拼写「中」，别的方案跟方案名。
    #[test]
    fn the_text_names_the_mode() {
        assert_eq!(
            status_text(&InputMode {
                english: true,
                scheme: "小鹤双拼".to_owned(),
                ..InputMode::default()
            }),
            "英"
        );
        assert_eq!(status_text(&InputMode::default()), "中");
        assert_eq!(
            status_text(&InputMode {
                scheme: "小鹤双拼".to_owned(),
                ..InputMode::default()
            }),
            "中 · 小鹤双拼"
        );
    }

    /// 画得出来、四周带阴影边，中与英画出来不一样。
    #[test]
    fn renders_the_hint() {
        let dir =
            std::env::temp_dir().join(format!("qingjian-wayland-hint-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let config = dir.join("config.toml");
        std::fs::write(&config, "[general]\nappearance = \"light\"\n").unwrap();
        let Some(mut painter) = Painter::new(config, None, Arc::new(Desktop::default())) else {
            eprintln!("没有系统字体，跳过");
            return;
        };
        let chinese = painter
            .render_status(&InputMode::default(), 1.5)
            .expect("中");
        assert!(chinese.content_x > 0 && chinese.content_width > 0);
        assert_eq!(chinese.next_frame, None, "提示没有动画");
        let english = InputMode {
            english: true,
            ..InputMode::default()
        };
        let english = painter.render_status(&english, 1.5).expect("英");
        assert_ne!(english.pixmap.data(), chinese.pixmap.data());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
