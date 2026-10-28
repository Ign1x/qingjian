//! 中英模式的归属：缺省所有会话共用一份；打开 `[general] remember_mode_per_app` 后按会话的应用（`OpenSession.app`）各记一份，
//! 没报应用名的会话各记各的。规则与 Windows Server 的 `dispatch/mode` 一致；切换走 `linux.rs` 的 `toggle_english`。

#[cfg(test)]
mod tests;

use qingjian_platform::protocol::SessionId;

use super::{Router, RouterConfig};

impl Router {
    /// 这个会话现在是不是英文模式；内置英文模式关着时一律中文。
    pub(super) fn english_of(&self, session: SessionId) -> bool {
        if !self.config.english_mode {
            return false;
        }
        if !self.config.remember_mode_per_app {
            return self.english;
        }
        let Some(info) = self.sessions.get(&session) else {
            return self.config.starts_english();
        };
        match app_key(info.app.as_deref()) {
            Some(app) => self
                .app_modes
                .get(&app)
                .copied()
                .unwrap_or(self.config.default_english),
            None => info.english,
        }
    }

    /// 记下这个会话切到的中 / 英：缺省记全局那一份，按应用记时记到它的应用上，没报应用名的记在会话自己身上。
    pub(super) fn set_english(&mut self, session: SessionId, english: bool) {
        let english = english && self.config.english_mode;
        if !self.config.remember_mode_per_app {
            self.english = english;
            return;
        }
        let Some(info) = self.sessions.get_mut(&session) else {
            return;
        };
        match app_key(info.app.as_deref()) {
            Some(app) => {
                self.app_modes.insert(app, english);
            }
            None => info.english = english,
        }
    }

    /// 热加载：只有模式策略（英文模式开关、默认英文、按应用记）变了才把记住的全部重置；
    /// 字体、标点这类无关的配置改动不能盖掉用户刚切出的模式。
    pub(super) fn reload_mode(&mut self, next: &RouterConfig) {
        if self.config.english_mode == next.english_mode
            && self.config.default_english == next.default_english
            && self.config.remember_mode_per_app == next.remember_mode_per_app
        {
            return;
        }
        let english = next.starts_english();
        self.english = english;
        self.app_modes.clear();
        for info in self.sessions.values_mut() {
            info.english = english;
        }
    }
}

/// 按应用记时用的键：应用名不区分大小写（`[apps]` 也这么认）；没报或报空串的不当应用，那个会话自己记。
fn app_key(app: Option<&str>) -> Option<String> {
    app.filter(|app| !app.is_empty()).map(str::to_lowercase)
}
