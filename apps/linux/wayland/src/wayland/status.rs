//! 输入状态的显示：回包上的模式交给候选窗的主题。

use qingjian_platform::protocol::linux::InputMode;

use super::Wayland;

impl Wayland {
    /// 输入模式变了：之后画的候选窗按它显示主题里的 `mode.*`。`None`（与 Server 断了）时候选窗沿用上一次的。
    pub(super) fn set_mode(&mut self, mode: Option<&InputMode>) {
        if let (Some(mode), Some(painter)) = (mode, &mut self.painter) {
            painter.set_mode(mode);
        }
    }
}
