//! 键盘摆进候选窗那一个 popup 的哪儿：合成器只给了光标旁的一个 popup，键盘只能与候选窗画进同一张位图，
//! 位置全在这里算（纯算术）。与 Windows 的 `key_hint/place.rs` 同一套思路，但候选窗只能往右下长（见 [`super::side`]），
//! 所以永远有两个放得下的侧边，不返回 `None`。

use super::side::{self, Side};

/// 键盘与候选窗之间的间隙（位图像素）：调用方按 4 逻辑像素乘缩放后四舍五入传进来。
///
/// # 参数
///
/// - `candidate`：候选窗内容区（卡片，不含阴影）左上角与宽高，位图像素；候选窗位图的原点在 (0, 0)。
/// - `keyboard`：键盘内容区宽、内容区高、位图左阴影（`content_x`）、位上阴影（`content_y`），位图像素。
/// - `flipped`：合成器把 popup 翻到了光标上方（候选窗在光标上面）。
/// - `sticky`：这段输入上一次用的那一侧；先用它，键盘不随光标左右跳。
/// - `gap`：键盘与候选窗之间的间隙，位图像素。
///
/// # 返回
///
/// `(用的侧, 候选窗位图原点的位移, 键盘内容区左上角在合成位图里的位置)`。
/// 键盘位图的原点由调用方按「内容位置 − 位图阴影边」摆（可为负，负的那一截贴图时裁掉）。
pub fn place(
    candidate: (i32, i32, u32, u32),
    keyboard: (u32, u32, u32, u32),
    flipped: bool,
    sticky: Option<Side>,
    gap: i32,
) -> (Side, (i32, i32), (i32, i32)) {
    // 两侧总放得下，先用上次那一侧，否则按翻转后的优先顺序取第一个
    let side = sticky.unwrap_or_else(|| side::order(flipped)[0]);
    let (candidate_x, candidate_y, candidate_width, candidate_height) = candidate;
    let (_width, height, _shadow_x, shadow_y) = keyboard;
    let (height, shadow_y) = (height as i32, shadow_y as i32);
    match side {
        Side::Right => (
            side,
            (0, 0),
            (candidate_x + candidate_width as i32 + gap, candidate_y),
        ),
        // 候选窗在光标下方：键盘再往下，候选窗不动
        Side::Away if !flipped => (
            side,
            (0, 0),
            (candidate_x, candidate_y + candidate_height as i32 + gap),
        ),
        // 候选窗在光标上方：键盘摆在上面（位图原点在 y = 0，内容区从位上阴影起），候选窗下移给它让位；
        // 位移后候选窗内容区正好在键盘内容区下沿 + gap，相对光标不跳
        Side::Away => (
            side,
            (0, shadow_y + height + gap - candidate_y),
            (candidate_x, shadow_y),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::place;
    use crate::view::Side;

    /// 键盘内容区 200 × 64、左上阴影 10 × 8；候选窗内容区 100 × 50。
    const KEYBOARD: (u32, u32, u32, u32) = (200, 64, 10, 8);

    /// 未翻转：先往右并排，候选窗不动，键盘内容区在候选窗右边一个间隙处。
    #[test]
    fn not_flipped_prefers_the_right() {
        let placed = place((0, 40, 100, 50), KEYBOARD, false, None, 4);
        assert_eq!(placed, (Side::Right, (0, 0), (104, 40)));
    }

    /// 翻转（候选窗在光标上方）：先往背向光标的那一侧，键盘摆在上面，候选窗下移 `上阴影 + 键盘高 + 间隙 − 候选窗上沿`，
    /// 让候选窗内容区正好在键盘内容区下沿 + 间隙处。
    #[test]
    fn flipped_prefers_away_and_moves_the_candidate_down() {
        let placed = place((0, 40, 100, 50), KEYBOARD, true, None, 4);
        assert_eq!(placed, (Side::Away, (0, 8 + 64 + 4 - 40), (0, 8)));
        // 候选窗内容区下移后：40 + 36 = 76 = 键盘内容区下沿 8 + 64 再加间隙 4
        assert_eq!(40 + placed.1.1, 8 + 64 + 4);
    }

    /// 这段输入上一次用的那一侧先试：粘在右边就一直往右，粘在背向光标那侧就一直往那侧，翻转与否都不换。
    #[test]
    fn the_previous_side_sticks() {
        assert_eq!(
            place((0, 40, 100, 50), KEYBOARD, true, Some(Side::Right), 4),
            (Side::Right, (0, 0), (104, 40))
        );
        assert_eq!(
            place((0, 40, 100, 50), KEYBOARD, false, Some(Side::Away), 4),
            (Side::Away, (0, 0), (0, 94))
        );
    }

    /// 间隙按传进来的算：两个方向都跟着挪。
    #[test]
    fn the_gap_counts() {
        assert_eq!(
            place((0, 40, 100, 50), KEYBOARD, false, Some(Side::Right), 10).2,
            (110, 40)
        );
        assert_eq!(
            place((0, 40, 100, 50), KEYBOARD, false, Some(Side::Away), 10).2,
            (0, 100)
        );
    }
}
