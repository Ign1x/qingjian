//! 键盘贴在候选窗的哪一侧。Windows 那份的 Left / Toward 去掉了：Wayland 的 popup 原点由合成器锚在光标旁，
//! 往左、往光标那一侧扩会把候选窗整体挪离光标；候选窗只能往右、往背向光标的那一侧长。

/// 键盘贴在候选窗的哪一侧。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// 右边：与候选窗并排，视线横着一扫就到。
    Right,

    /// 背向光标：候选窗在光标下方就再往下，在上方就再往上。
    Away,
}

/// 没有上一次用的侧边时的优先顺序：先往右并排，放不下再往背向光标那一侧。
/// `flipped` 是合成器把 popup 翻到了光标上方（候选窗在光标上方），这时往下（`Away`）先试，免得把 popup 顶出屏幕。
pub fn order(flipped: bool) -> [Side; 2] {
    if flipped {
        [Side::Away, Side::Right]
    } else {
        [Side::Right, Side::Away]
    }
}

#[cfg(test)]
mod tests {
    use super::{Side, order};

    /// 未翻转先往右，翻转（候选窗在光标上方）先往背向光标的那一侧；两种顺序都含两侧。
    #[test]
    fn flipped_tries_away_first() {
        assert_eq!(order(false), [Side::Right, Side::Away]);
        assert_eq!(order(true), [Side::Away, Side::Right]);
    }
}
