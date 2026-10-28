//! Server 回包上的输入模式 → 渲染器的输入状态：主题的 `mode.*` 照它显示中 / 英、简 / 繁、标点与方案。

use qingjian_render::Mode;

use crate::protocol::linux::InputMode;

/// 回包的 `mode` 换成渲染器要的 [`Mode`]。方案名用总有内容的那个（单开全拼也写「全拼」），与 Windows 填主题的一致。
pub fn render_mode(mode: &InputMode) -> Mode {
    Mode {
        english: mode.english,
        traditional: mode.traditional,
        full_width: mode.full_width,
        scheme: mode.scheme_name().to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 三个布尔原样搬，单开全拼时方案名补成「全拼」，双拼照状态条的写法。
    #[test]
    fn mode_carries_over() {
        let mode = render_mode(&InputMode {
            english: true,
            traditional: true,
            full_width: false,
            scheme: String::new(),
        });
        assert!(mode.english && mode.traditional && !mode.full_width);
        assert_eq!(mode.scheme, "全拼");
        let shuangpin = render_mode(&InputMode {
            scheme: "小鹤双拼".to_owned(),
            ..InputMode::default()
        });
        assert_eq!(shuangpin.scheme, "小鹤双拼");
    }
}
