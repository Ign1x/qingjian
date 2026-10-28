//! 应用选区里的文字：选区翻译时前端从应用给的那段前后文里取出来，回给 Server。

/// 选区最多多少个字：再长 Server 不翻译、把快捷键放行（`apps/linux/server` 的 `receive_selection`），
/// 前端也就不读，回空串。与 Fcitx5 插件（`apps/linux/surrounding.h` 的 `selected`）同一个数。
pub const MAX_SELECTION_CHARS: usize = 500;

/// 应用给的一段文字里选中的那部分：`cursor` / `anchor` 是 **UTF-8 字节偏移**（Wayland text-input 的单位），两者之间是选区。
///
/// 没有选区（两者相等）、偏移越界或落在字符中间、选区超过 [`MAX_SELECTION_CHARS`] 个字时返回 `None`：
/// 超长的不截一段去翻译。规则与 Fcitx5 插件的 `qingjian::selected` 一样，只是那边的偏移按字符数。
pub fn selected_text(text: &str, cursor: usize, anchor: usize) -> Option<&str> {
    if cursor == anchor {
        return None;
    }
    let selected = text.get(cursor.min(anchor)..cursor.max(anchor))?;
    (selected.chars().count() <= MAX_SELECTION_CHARS).then_some(selected)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 光标在选区哪一头都一样；偏移按字节，emoji 四字节。
    #[test]
    fn either_end_may_hold_the_cursor() {
        let text = "我喜欢😀苹果。";
        // 「我喜欢」9 字节、😀 4 字节，「苹果」从 13 到 19
        assert_eq!(selected_text(text, 19, 13), Some("苹果"));
        assert_eq!(selected_text(text, 13, 19), Some("苹果"));
        assert_eq!(selected_text(text, 9, 13), Some("😀"));
    }

    /// 没有选区、越界、落在字符中间都取不出来。
    #[test]
    fn unusable_offsets_give_nothing() {
        assert_eq!(selected_text("你好", 3, 3), None);
        assert_eq!(selected_text("你好", 3, 7), None);
        assert_eq!(selected_text("你好", 1, 6), None);
        assert_eq!(selected_text("", 0, 0), None);
    }

    /// 500 个字还读，501 个字整个不读，不截半段。
    #[test]
    fn long_selections_are_not_read() {
        let limit = "字".repeat(MAX_SELECTION_CHARS);
        assert_eq!(selected_text(&limit, 0, limit.len()), Some(limit.as_str()));
        let longer = "字".repeat(MAX_SELECTION_CHARS + 1);
        assert_eq!(selected_text(&longer, longer.len(), 0), None);
    }
}
