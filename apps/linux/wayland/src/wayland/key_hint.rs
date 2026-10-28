//! 键位提示：Server 回包上的 `key_hint` 换成画好的键盘，与候选窗合成进同一个 popup。
//! 唯一入口是 [`Wayland::set_key_hint`]，会话那一侧只经 `Output::key_hint` 调它。

use qingjian_platform::protocol::linux::KeyHintView;

use super::Wayland;

impl Wayland {
    /// 换掉候选窗旁画着的键位提示：`None` 收起键盘（原来有键盘才重画）；`Some` 重画一块键盘，
    /// 不透明度已经乘进位图。画不出来时记日志、留着原来的键盘。
    pub(super) fn set_key_hint(&mut self, hint: Option<&KeyHintView>) {
        let Some(hint) = hint else {
            self.key_hint = None;
            self.keyboard_side = None;
            if self.keyboard.take().is_some() {
                self.paint();
            }
            return;
        };
        let scale = self.popup.scale() as f32;
        let Some(rendered) = self
            .painter
            .as_mut()
            .and_then(|painter| painter.render_key_hint(hint, scale))
        else {
            tracing::warn!("键位提示的键盘画不出来，这次不显示");
            self.key_hint = None;
            self.keyboard_side = None;
            if self.keyboard.take().is_some() {
                self.paint();
            }
            return;
        };
        self.key_hint = Some(hint.clone());
        self.keyboard = Some(rendered);
        self.paint();
    }
}
