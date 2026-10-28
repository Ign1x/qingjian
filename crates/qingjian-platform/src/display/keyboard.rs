//! 键位提示 → 渲染器的键盘：Core 的双拼键位、五笔字根图换成 [`Keyboard`]，Linux 线上的 [`KeyHintContent`] 也在这里换。
//! 照 Windows Server 的 `ui/candidates/key_hint/keyboard.rs` 写（与 macOS 壳 `host::presenting::key_hint` 同一套），那两份以后改用这里。

use qingjian_core::root_chart::RootChart;
use qingjian_core::shuangpin::{KeyHint, KeyState};
use qingjian_render::{Keyboard, Keycap, KeycapState, Root};

use crate::protocol::linux::KeyHintContent;

/// 回包上的键位提示换成键盘。字根图每次都要解析全部字形，前端拿到同一张图时留着上次的复用。
pub fn key_hint_keyboard(content: &KeyHintContent) -> Keyboard {
    match content {
        KeyHintContent::Shuangpin(hint) => shuangpin_keyboard(hint),
        KeyHintContent::Roots(id) => root_keyboard(id.chart()),
    }
}

/// 双拼键位：键名大写，翘舌声母是角标，下方几行是这个键当韵母时的写法。
pub fn shuangpin_keyboard(hint: &KeyHint) -> Keyboard {
    let rows = hint
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|cap| Keycap {
                    label: cap.key.to_ascii_uppercase().to_string(),
                    corner: cap.initial.clone(),
                    lines: cap.finals.clone(),
                    state: match &cap.state {
                        KeyState::Idle => KeycapState::Normal,
                        KeyState::Pressed => KeycapState::Pressed,
                        KeyState::Next(syllable) => KeycapState::Next(syllable.clone()),
                        KeyState::Unavailable => KeycapState::Dimmed,
                    },
                    roots: Vec::new(),
                })
                .collect()
        })
        .collect();
    Keyboard { rows }
}

/// 字根图：每键底下一行键名与区位号，Z 是万能键。
pub fn root_keyboard(chart: &RootChart) -> Keyboard {
    let rows = chart
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|key| Keycap {
                    label: key.key.to_string(),
                    corner: key.zone.map(|zone| zone.to_string()),
                    lines: if key.roots.is_empty() {
                        vec!["万能键".to_owned()]
                    } else {
                        Vec::new()
                    },
                    roots: key
                        .roots
                        .iter()
                        .filter_map(|glyph| Root::from_svg(glyph.svg, glyph.frame))
                        .collect(),
                    ..Keycap::default()
                })
                .collect()
        })
        .collect();
    Keyboard { rows }
}

#[cfg(test)]
mod tests {
    use qingjian_core::root_chart::RootChart;
    use qingjian_core::shuangpin::{KeyHint, Scheme};
    use qingjian_render::{Keyboard, Keycap, KeycapState};

    use super::{key_hint_keyboard, root_keyboard, shuangpin_keyboard};
    use crate::protocol::linux::{KeyHintContent, RootChartId};

    fn key<'a>(keyboard: &'a Keyboard, label: &str) -> &'a Keycap {
        keyboard
            .rows
            .iter()
            .flatten()
            .find(|cap| cap.label == label)
            .unwrap()
    }

    /// 小鹤敲了 `v`（zh）：能接成音节的键标出音节，接不成的变暗；键名大写，翘舌声母当角标。
    #[test]
    fn shuangpin_keys_carry_their_next_syllable() {
        let keyboard = shuangpin_keyboard(&KeyHint::for_keys(Scheme::Xiaohe.layout(), "nihcv"));
        assert_eq!(
            keyboard.rows.iter().map(Vec::len).collect::<Vec<_>>(),
            [10, 9, 7]
        );
        assert_eq!(
            key(&keyboard, "S").state,
            KeycapState::Next("zhong".to_owned())
        );
        assert_eq!(key(&keyboard, "B").state, KeycapState::Dimmed);
        let v = key(&keyboard, "V");
        assert_eq!(v.corner.as_deref(), Some("zh"));
        assert_eq!(v.lines, ["ui", "ü"]);
        assert!(
            keyboard
                .rows
                .iter()
                .flatten()
                .all(|cap| cap.roots.is_empty())
        );

        let idle = shuangpin_keyboard(&KeyHint::for_keys(Scheme::Xiaohe.layout(), ""));
        assert!(
            idle.rows
                .iter()
                .flatten()
                .all(|cap| cap.state == KeycapState::Normal)
        );
    }

    /// 字根图的每个字形都解析得出来；Z 是万能键，只写一行字。
    #[test]
    fn root_chart_keys_carry_every_glyph() {
        let chart = RootChart::wubi86();
        let keyboard = root_keyboard(chart);
        let glyphs = chart
            .rows
            .iter()
            .flatten()
            .map(|key| key.roots.len())
            .sum::<usize>();
        let roots = keyboard
            .rows
            .iter()
            .flatten()
            .map(|cap| cap.roots.len())
            .sum::<usize>();
        assert_eq!(roots, glyphs);
        let z = key(&keyboard, "Z");
        assert_eq!(
            (z.corner.as_deref(), z.lines.as_slice()),
            (None, &["万能键".to_owned()][..])
        );
        let g = key(&keyboard, "G");
        assert_eq!(g.corner.as_deref(), Some("11"));
        assert!(g.lines.is_empty() && !g.roots.is_empty());
    }

    #[test]
    fn wire_content_picks_the_keyboard() {
        let hint = KeyHint::for_keys(Scheme::Xiaohe.layout(), "k");
        assert_eq!(
            key_hint_keyboard(&KeyHintContent::Shuangpin(hint.clone())),
            shuangpin_keyboard(&hint)
        );
        assert_eq!(
            key_hint_keyboard(&KeyHintContent::Roots(RootChartId::Wubi86)),
            root_keyboard(RootChart::wubi86())
        );
    }
}
