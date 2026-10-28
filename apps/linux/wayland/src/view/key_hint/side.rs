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

/// 没有上一次用的侧边时的优先顺序：竖排先往右并排，放不下再往背向光标那一侧。
/// `flipped` 是合成器把 popup 翻到了光标上方（候选窗在光标上方），这时往上（`Away`）先试，免得把 popup 顶出屏幕；
/// `bar` 是候选窗是一长条（横排单行）：再往右并排要占两者宽度之和，容易出屏幕右边，
/// popup 协议又给不出屏幕还剩多宽（sway 不会把出界的 popup 往回推），所以也先往背向光标那一侧。
pub fn order(flipped: bool, bar: bool) -> [Side; 2] {
    if flipped || bar {
        [Side::Away, Side::Right]
    } else {
        [Side::Right, Side::Away]
    }
}

#[cfg(test)]
mod tests {
    use super::{Side, order};

    /// 竖排未翻转先往右；翻转（候选窗在光标上方）或横排一长条先往背向光标的那一侧；两种顺序都含两侧。
    #[test]
    fn flipped_or_bar_tries_away_first() {
        assert_eq!(order(false, false), [Side::Right, Side::Away]);
        assert_eq!(order(true, false), [Side::Away, Side::Right]);
        assert_eq!(order(false, true), [Side::Away, Side::Right]);
        assert_eq!(order(true, true), [Side::Away, Side::Right]);
    }
}
