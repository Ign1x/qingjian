//! 输入模式：回包上附的 `mode`，以及托盘切的全角标点与简繁（中 / 英与切换键同一条路，在 `linux.rs`）。
//! 后两样是全局设置，与 Windows 状态条一样当场改内存、写回配置文件，热加载再读回来的是同一个值。

use qingjian_core::TraditionalVariant;
use qingjian_platform::Config;
use qingjian_platform::protocol::SessionId;
use qingjian_platform::protocol::linux::InputMode;

use super::Router;

impl Router {
    /// 这个会话现在的输入模式，附在发给它的每个 `KeyResult` / `Update` 上。
    pub(super) fn input_mode(&self, session: SessionId) -> Option<InputMode> {
        if !self.sessions.contains_key(&session) {
            return None;
        }
        let english = self.english_of(session);
        Some(InputMode {
            english,
            traditional: self.engine.traditional() != TraditionalVariant::Off,
            full_width: self.full_width_for(english),
            scheme: self.scheme_label(),
        })
    }

    /// 翻转这个会话当前模式那一份全角标点（中英各记一份）。
    pub(super) fn toggle_punctuation(&mut self, session: SessionId) {
        let english = self.english_of(session);
        let full_width = !self.full_width_for(english);
        let key = if english {
            self.config.english_full_width = full_width;
            "english_full_width_punctuation"
        } else {
            self.config.full_width = full_width;
            "full_width_punctuation"
        };
        tracing::debug!(english, full_width, "切换全角标点");
        self.persist_flag(key, full_width);
    }

    /// 在简体与上次用的繁体之间切换输出字形（同 Windows 的简繁快捷键）；组句中的候选当场换字形。
    pub(super) fn toggle_traditional(&mut self) {
        let current = self.engine.traditional();
        let next = if current == TraditionalVariant::Off {
            self.config.last_traditional
        } else {
            self.config.last_traditional = current;
            self.persist_variant("last_traditional", current);
            TraditionalVariant::Off
        };
        self.engine.set_traditional(next);
        self.persist_variant("traditional_variant", next);
        tracing::debug!(traditional = next.key(), "切换简繁输出");
        if !self.engine.composition().is_empty() {
            self.recompose(None);
        }
    }

    /// 写回 `[general]` 的一个开关；没开热加载（测试）就只改内存。
    fn persist_flag(&self, key: &str, value: bool) {
        if let Some(path) = self.config_path()
            && let Err(error) = Config::set_value(path, "general", key, value)
        {
            tracing::warn!(%error, key, "写回配置失败");
        }
    }

    /// 同 [`Self::persist_flag`]，写的是繁体字形。
    fn persist_variant(&self, key: &str, value: TraditionalVariant) {
        if let Some(path) = self.config_path()
            && let Err(error) = Config::set_value(path, "general", key, value.key())
        {
            tracing::warn!(%error, key, "写回配置失败");
        }
    }
}
