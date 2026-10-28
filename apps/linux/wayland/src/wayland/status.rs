//! 输入状态的显示：回包上的模式交给候选窗的主题与托盘；切中 / 英时在候选窗的位置亮一下提示。

use qingjian_platform::protocol::linux::InputMode;

use super::state::Wayland;

impl Wayland {
    /// 输入模式变了：之后画的候选窗按它显示主题里的 `mode.*`，托盘换图标与菜单的勾。
    /// `None`（与 Server 断了）时候选窗沿用上一次的，托盘换回不带中 / 英的图标。
    pub(super) fn set_mode(&mut self, mode: Option<&InputMode>) {
        if let (Some(mode), Some(painter)) = (mode, &mut self.painter) {
            painter.set_mode(mode);
        }
        if let Some(tray) = &self.tray {
            tray.show(mode);
        }
    }

    /// 在候选窗的 popup 里画中 / 英提示；字体库没建起来时画不了，返回 `false`。收起走 [`Wayland::hide`]。
    pub(super) fn hint(&mut self, mode: &InputMode) -> bool {
        let scale = self.popup.scale();
        let Some(rendered) = self
            .painter
            .as_mut()
            .and_then(|painter| painter.render_status(mode, scale as f32))
        else {
            return false;
        };
        // popup 上换成了提示：缩放、主题变了时不再拿候选那一帧重画
        self.shown = None;
        self.paint(&rendered);
        true
    }
}
