//! 应用里的行内拼音，以及上屏文字怎么切块。

use qingjian_platform::protocol::Frame;

/// input-method-v2 一条消息里的文字上限（协议写明 4000 字节；整条消息超过约 4 KB 合成器直接断开连接）。
const MAX_BYTES: usize = 4000;

/// 放进应用的行内拼音：文字与光标（UTF-8 字节偏移，协议的单位）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Preedit {
    pub text: String,

    pub cursor: i32,
}

impl Preedit {
    /// 帧里该放进应用的那一串：`[general] preedit = "window"` 时为空（只在候选窗里画）。
    /// 帧的光标按字符数，这里换成字节偏移；太长的截到 4000 字节。
    pub fn from_frame(frame: &Frame) -> Self {
        if !frame.preedit_mode.inline() {
            return Self::default();
        }
        let joined: String = frame
            .preedit
            .iter()
            .map(|segment| segment.text.as_str())
            .collect();
        let text = chunks(&joined).into_iter().next().unwrap_or_default();
        let cursor = text
            .char_indices()
            .nth(frame.cursor)
            .map_or(text.len(), |(index, _)| index);
        Self {
            text: text.to_owned(),
            cursor: cursor as i32,
        }
    }
}

/// 按字符边界切成不超过 4000 字节的块；空串没有块。
pub fn chunks(text: &str) -> Vec<&str> {
    let mut chunks = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let mut end = rest.len().min(MAX_BYTES);
        while !rest.is_char_boundary(end) {
            end -= 1;
        }
        chunks.push(&rest[..end]);
        rest = &rest[end..];
    }
    chunks
}

#[cfg(test)]
mod tests {
    use qingjian_platform::PreeditMode;
    use qingjian_platform::protocol::{PreeditKind, PreeditSegment};

    use super::*;

    fn frame(mode: PreeditMode, cursor: usize) -> Frame {
        Frame {
            preedit: ["你好", "shi'jie"]
                .into_iter()
                .map(|text| PreeditSegment {
                    text: text.to_owned(),
                    kind: PreeditKind::Typed,
                })
                .collect(),
            preedit_mode: mode,
            cursor,
            ..Frame::default()
        }
    }

    /// 光标按字符数给，换成字节：「你好」两个字是 6 字节。
    #[test]
    fn cursor_turns_into_a_byte_offset() {
        let preedit = Preedit::from_frame(&frame(PreeditMode::Both, 2));
        assert_eq!(preedit.text, "你好shi'jie");
        assert_eq!(preedit.cursor, 6);
        assert_eq!(
            Preedit::from_frame(&frame(PreeditMode::Inline, 9)).cursor,
            13
        );
        assert_eq!(
            Preedit::from_frame(&frame(PreeditMode::Both, 99)).cursor,
            13
        );
    }

    /// 只在候选窗里显示拼音时应用里什么都不放。
    #[test]
    fn window_only_mode_keeps_the_app_clean() {
        assert_eq!(
            Preedit::from_frame(&frame(PreeditMode::Window, 2)),
            Preedit::default()
        );
    }

    /// 长文字按字符边界切成不超过 4000 字节的块，拼回去一字不差。
    #[test]
    fn long_text_is_cut_at_char_boundaries() {
        assert!(chunks("").is_empty());
        assert_eq!(chunks("你好"), ["你好"]);
        let long = "字".repeat(3000);
        let parts = chunks(&long);
        assert_eq!(parts.len(), 3);
        assert!(parts.iter().all(|part| part.len() <= MAX_BYTES));
        assert_eq!(parts[0].len(), 3999);
        assert_eq!(parts.concat(), long);
    }
}
