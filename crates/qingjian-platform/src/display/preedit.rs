//! 候选窗口顶部的拼音行。

use qingjian_render::{Preedit, PreeditSegment, PreeditStyle};

use crate::protocol::{Frame, PreeditKind};

/// 窗口顶部要画的拼音行：`[general] preedit` 配成「只在行内」时没有（拼音已经在应用里）。
/// 带调拼音时窗口里画另一串（`window_preedit`），应用里的行内拼音仍是 `preedit`。
pub fn window_preedit(frame: &Frame) -> Option<Preedit> {
    if !frame.preedit_mode.in_window() {
        return None;
    }
    let (segments, cursor) = frame
        .window_preedit
        .as_deref()
        .map_or((&frame.preedit, frame.cursor), |(segments, cursor)| {
            (segments, *cursor)
        });
    if segments.is_empty() {
        return None;
    }
    Some(Preedit {
        segments: segments
            .iter()
            .map(|segment| PreeditSegment {
                text: segment.text.clone(),
                style: match segment.kind {
                    PreeditKind::Typed => PreeditStyle::Typed,
                    PreeditKind::Rest => PreeditStyle::Rest,
                    PreeditKind::Corrected => PreeditStyle::Struck,
                    PreeditKind::AuxCode => PreeditStyle::AuxCode,
                },
            })
            .collect(),
        cursor,
    })
}

#[cfg(test)]
mod tests {
    use crate::PreeditMode;
    use crate::protocol::{Frame, PreeditKind, PreeditSegment};

    use super::window_preedit;

    fn frame(mode: PreeditMode) -> Frame {
        Frame {
            preedit: vec![PreeditSegment {
                text: "ni'hao".to_owned(),
                kind: PreeditKind::Typed,
            }],
            preedit_mode: mode,
            cursor: 6,
            ..Frame::default()
        }
    }

    /// 只有「只在行内」不给窗口拼音行；另两档窗口都得画。
    #[test]
    fn window_keeps_the_pinyin_row_unless_inline_only() {
        assert!(window_preedit(&frame(PreeditMode::Both)).is_some());
        assert!(window_preedit(&frame(PreeditMode::Window)).is_some());
        assert!(window_preedit(&frame(PreeditMode::Inline)).is_none());
    }

    /// 带调拼音另给一串时窗口画它，光标也用它的。
    #[test]
    fn toned_pinyin_replaces_the_window_row() {
        let mut frame = frame(PreeditMode::Both);
        frame.window_preedit = Some(Box::new((
            vec![PreeditSegment {
                text: "nǐ'hǎo".to_owned(),
                kind: PreeditKind::Typed,
            }],
            3,
        )));
        let preedit = window_preedit(&frame).expect("有拼音行");
        assert_eq!(preedit.segments[0].text, "nǐ'hǎo");
        assert_eq!(preedit.cursor, 3);
    }
}
