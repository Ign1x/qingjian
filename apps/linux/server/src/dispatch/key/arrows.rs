//! 组句中的方向键：`[shortcut] highlight_keys` 决定高亮由 `↑` / `↓` 还是 `←` / `→` 移动；
//! 横排且选了 `←` / `→` 时分拼音行与候选两区，怎么走照 Core 的 `arrows::route`（与 macOS 壳、Windows Server 同一份规则）。

use qingjian_core::candidate::arrows::{self, Arrow, ArrowAction, Matrix};
use qingjian_platform::HighlightKeys;

use super::Effect;
use crate::dispatch::Router;

impl Router {
    /// 方向键分两区时（`RouterConfig::two_area_arrows`）按 `arrows::route` 在拼音行与候选之间走。没有候选时 ← / → 照旧移拼音光标。
    pub(super) fn two_area_key(&mut self, arrow: Arrow) -> Effect {
        if self.candidate_count() == 0 {
            return match arrow {
                Arrow::Left | Arrow::Right => {
                    self.move_caret(if arrow == Arrow::Left { -1 } else { 1 });
                    Effect::Changed(None)
                }
                Arrow::Up | Arrow::Down => Effect::Navigated,
            };
        }
        match arrows::route(arrow, self.focus, Matrix::Off) {
            ArrowAction::MoveCursor(delta) => {
                self.move_caret(delta);
                return Effect::Changed(None);
            }
            ArrowAction::MoveHighlight(delta) => self.move_highlight(delta),
            ArrowAction::Focus(focus) => self.focus = focus,
            ArrowAction::MoveRow(_) | ArrowAction::Expand | ArrowAction::Stay => {}
        }
        Effect::Navigated
    }

    /// `↑` / `↓`：按 `[shortcut] highlight_keys` 移高亮。高亮键配成 `←` / `→` 时它俩没活干，
    /// 这一下吃掉但不动——不吃掉会漏给应用去挪插入符。
    pub(super) fn vertical_key(&mut self, step: isize) -> Effect {
        if self.config.highlight_keys == HighlightKeys::UpDown {
            self.move_highlight(step);
        }
        Effect::Navigated
    }

    /// `←` / `→`：缺省只走拼音光标；高亮键配成 `←` / `→` 时两键一起管，把「拼音行 + 候选栏」当成一条从左到右的序列走：
    /// 拼音行在左边，所以 `→` 先走光标、到末位溢出成选词，`←` 先退候选、到首位回光标（与 macOS 壳的 `move_cells` 同一套）。
    pub(super) fn horizontal_key(&mut self, step: isize) -> Effect {
        if self.config.highlight_keys == HighlightKeys::UpDown {
            self.move_caret(step);
            return Effect::Changed(None);
        }
        if step > 0 {
            if self.move_caret(1) {
                return Effect::Changed(None);
            }
            self.move_highlight(1);
            return Effect::Navigated;
        }
        if self.highlight > 0 {
            self.move_highlight(-1);
            return Effect::Navigated;
        }
        // 高亮已在首位：把这一下还给拼音光标；光标也在开头时没有去处，照样吃掉
        if self.move_caret(-1) {
            return Effect::Changed(None);
        }
        Effect::Navigated
    }

    /// 拼音光标左右移一格。已在开头 / 末尾时返回 `false`，调用方据此溢出到另一个动作。
    fn move_caret(&mut self, step: isize) -> bool {
        if step < 0 {
            self.engine.move_cursor_left()
        } else {
            self.engine.move_cursor_right()
        }
    }
}
