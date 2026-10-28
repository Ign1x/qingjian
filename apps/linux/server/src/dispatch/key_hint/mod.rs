//! 键位提示（`[general] key_hint`）：照 Windows Server 的 `dispatch/key_hint` 算这一帧带不带键盘，开着五笔带字根图（混输也是），否则带双拼键位。
//! 只给握手报了 `features.key_hint` 的会话（自己画候选窗的前端），由 `display` 附在回包的 `key_hint` 上；键盘怎么画、摆在哪由前端定。

mod state;

#[cfg(test)]
mod tests;

use qingjian_platform::KeyHintMode;
use qingjian_platform::protocol::SessionId;
use qingjian_platform::protocol::linux::{KeyHintContent, KeyHintView, RootChartId};

pub(super) use self::state::KeyHintState;
use super::Router;

impl Router {
    /// 发给 `session` 的这次回包要附的键位提示：前端握手报了画得了、焦点在这个会话上才有。
    pub(super) fn key_hint_for(&mut self, session: SessionId) -> Option<KeyHintView> {
        let info = self.sessions.get(&session)?;
        if !info.features.key_hint || info.disabled || self.focused != Some(session) {
            return None;
        }
        self.key_hint_view()
    }

    /// 这一帧的键位提示；不该画时为 `None`。组句结束（缓冲区空了）时清掉「卡住时显示」的状态。
    fn key_hint_view(&mut self) -> Option<KeyHintView> {
        let composing = self.config.key_hint != KeyHintMode::Off
            && self.translation.is_none()
            && !self.engine.composition().is_empty();
        let chart = composing.then(|| self.engine.root_chart()).flatten();
        let hint = (composing && chart.is_none())
            .then(|| self.engine.key_hint())
            .flatten();
        // 认不出的字根图（Core 加了新的、线上标识还没跟上）宁可不画，不画错
        let content = match (chart.and_then(RootChartId::of), hint) {
            (Some(id), _) => KeyHintContent::Roots(id),
            (None, Some(hint)) => KeyHintContent::Shuangpin(hint),
            (None, None) => {
                self.key_hint = KeyHintState::default();
                return None;
            }
        };
        // 字根图上没有「拼不成」的说法，只等停顿
        let stuck = matches!(&content, KeyHintContent::Shuangpin(hint) if hint.stuck);
        let show = match self.config.key_hint {
            KeyHintMode::Always => true,
            KeyHintMode::Auto => self
                .key_hint
                .update(self.engine.composition().text(), stuck),
            KeyHintMode::Off => false,
        };
        let (opacity, size) = self.config.key_hint_look;
        show.then_some(KeyHintView {
            content,
            opacity,
            size,
        })
    }

    /// `tick` 里调：「卡住时显示」的停顿到点了就记成弹出过，下一次轮询的回包带上键盘。
    pub(super) fn advance_key_hint(&mut self) {
        if !self.engine.composition().is_empty() {
            self.key_hint.advance();
        }
    }
}
