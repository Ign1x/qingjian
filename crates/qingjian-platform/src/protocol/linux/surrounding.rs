//! 前端传给 Linux Server 的应用光标前后文（云联想拿它当上文）。
use qingjian_core::SurroundingText;
use serde::{Deserialize, Serialize};

/// 光标两侧各取多少个字。与 Fcitx5 插件（`apps/linux/surrounding.h`）一致。
const SIDE_CHARS: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSurrounding {
    pub before: String,

    pub after: String,
}

impl AppSurrounding {
    /// 从应用给的一段文字截出光标两侧：`cursor` / `anchor` 是 **UTF-8 字节偏移**（Wayland text-input 的单位），
    /// 两者不等时中间是选区，跳过不要；两侧各取至多 80 个字。偏移越界或落在字符中间时返回 `None`。
    ///
    /// 规则与 Fcitx5 插件的 `qingjian::surrounding` 一样，只是那边的偏移按字符数。
    pub fn around(text: &str, cursor: usize, anchor: usize) -> Option<Self> {
        let start = cursor.min(anchor);
        let end = cursor.max(anchor);
        if end > text.len() || !text.is_char_boundary(start) || !text.is_char_boundary(end) {
            return None;
        }
        let head = &text[..start];
        let before_from = head
            .char_indices()
            .rev()
            .nth(SIDE_CHARS - 1)
            .map_or(0, |(index, _)| index);
        let tail = &text[end..];
        let after_to = tail
            .char_indices()
            .nth(SIDE_CHARS)
            .map_or(tail.len(), |(index, _)| index);
        Some(Self {
            before: head[before_from..].to_owned(),
            after: tail[..after_to].to_owned(),
        })
    }
}

impl From<AppSurrounding> for SurroundingText {
    fn from(value: AppSurrounding) -> Self {
        Self {
            before: value.before,
            after: value.after,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 与 `apps/linux/tests/surrounding.cpp` 同几条，偏移换成字节：选区跳过、正反向一样、emoji 四字节。
    #[test]
    fn selection_is_skipped_in_either_direction() {
        let text = "你好😀选中字世界";
        // 「你好😀」是 3 + 3 + 4 = 10 字节，「选中字」再 9 字节
        let selected = AppSurrounding::around(text, 10, 19).expect("偏移合法");
        assert_eq!(selected.before, "你好😀");
        assert_eq!(selected.after, "世界");
        assert_eq!(AppSurrounding::around(text, 19, 10), Some(selected));
    }

    /// 越界与落在字符中间都不要。
    #[test]
    fn bad_offsets_are_rejected() {
        assert_eq!(AppSurrounding::around("你好", 7, 7), None);
        assert_eq!(AppSurrounding::around("你好", 1, 1), None);
        assert_eq!(AppSurrounding::around("你好", 3, 7), None);
    }

    /// 空串合法；两侧各至多 80 个字。
    #[test]
    fn sides_are_capped_at_eighty_chars() {
        let empty = AppSurrounding::around("", 0, 0).expect("空串");
        assert_eq!((empty.before.as_str(), empty.after.as_str()), ("", ""));
        let many = "a".repeat(100);
        assert_eq!(
            AppSurrounding::around(&many, 90, 90).expect("中间").before,
            "a".repeat(80)
        );
        assert_eq!(
            AppSurrounding::around(&many, 0, 0).expect("开头").after,
            "a".repeat(80)
        );
        let wide = "字".repeat(100);
        let around = AppSurrounding::around(&wide, 150, 150).expect("中间");
        assert_eq!(around.before, "字".repeat(50));
        assert_eq!(around.after, "字".repeat(50));
        let around = AppSurrounding::around(&wide, 270, 270).expect("靠后");
        assert_eq!(around.before.chars().count(), 80);
    }
}
