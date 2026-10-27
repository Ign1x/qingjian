//! 输入模式：附在回包上的 `mode`。

use qingjian_core::TraditionalVariant;
use qingjian_platform::protocol::SessionId;
use qingjian_platform::protocol::linux::InputMode;
use qingjian_platform::scheme_label;

use super::Router;

impl Router {
    /// 这个会话现在的输入模式，附在发给它的每个 `KeyResult` / `Update` 上。
    pub(super) fn input_mode(&self, session: SessionId) -> Option<InputMode> {
        let english = self.sessions.get(&session)?.english;
        Some(InputMode {
            english,
            traditional: self.engine.traditional() != TraditionalVariant::Off,
            full_width: self.full_width_for(english),
            // Linux 还不装用户双拼与形码方案（没有 SchemeSlot），按配置的拼音侧算，与引擎实际跑的一致
            scheme: scheme_label(self.config.scheme, None, None),
        })
    }
}
