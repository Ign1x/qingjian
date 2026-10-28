//! 候选窗跟着输入模式画：主题里的 `mode.*`（中 / 英、简 / 繁、标点、方案）照 Server 回包上的模式显示。

use qingjian_platform::display;
use qingjian_platform::protocol::linux::InputMode;

use super::Painter;

impl Painter {
    /// 换上 Server 回包上的模式，之后画的帧按它显示；返回变没变（变了时画着的那一帧要不要重画由会话定）。
    pub fn set_mode(&mut self, mode: &InputMode) -> bool {
        let mode = display::render_mode(mode);
        if mode == self.mode {
            return false;
        }
        self.mode = mode;
        true
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use qingjian_core::{Candidate, CandidateKind, CandidateList};
    use qingjian_platform::Appearance;
    use qingjian_platform::protocol::Frame;
    use qingjian_platform::protocol::linux::InputMode;

    use super::super::Painter;
    use crate::portal::Desktop;

    /// 初音未来主题在候选窗角上按 `mode.english` 写「中」或「英」：换了模式画出来就不一样，同一个模式再设一次不算变。
    #[test]
    fn the_theme_shows_the_mode() {
        let dir =
            std::env::temp_dir().join(format!("qingjian-wayland-mode-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let config = dir.join("config.toml");
        std::fs::write(&config, "[general]\ntheme = \"miku\"\nanimations = false\n").unwrap();
        let Some(mut painter) = Painter::new(config, None, Arc::new(Desktop::default())) else {
            eprintln!("没有系统字体，跳过");
            return;
        };
        let frame = Frame {
            candidates: CandidateList {
                items: vec![Candidate {
                    text: "你好".to_owned(),
                    kind: CandidateKind::Chinese,
                    syllables: Vec::new(),
                    reading: None,
                    translation: None,
                    aux_code: None,
                }],
            },
            appearance: Appearance::Light,
            page_count: 1,
            ..Frame::default()
        };
        let chinese = painter.render(&frame, 1.0).unwrap();
        let english = InputMode {
            english: true,
            ..InputMode::default()
        };
        assert!(painter.set_mode(&english));
        assert!(!painter.set_mode(&english), "同一个模式再设一次不算变");
        painter.forget();
        let english = painter.render(&frame, 1.0).unwrap();
        assert_ne!(english.pixmap.data(), chinese.pixmap.data());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
