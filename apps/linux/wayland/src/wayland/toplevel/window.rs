//! 一个顶层窗口：生效的 app_id 与激活状态，以及等 `done` 生效的那一份。

#[derive(Debug, Default)]
pub struct Window {
    /// 生效的 app_id；还没报过是 `None`。
    pub app: Option<String>,

    /// 激活着时是第几次激活（比先后用）；没激活是 `None`。
    pub activated: Option<u64>,

    /// 下一次 `done` 生效的 app_id。
    pub pending_app: Option<String>,

    /// 下一次 `done` 生效的激活状态。
    pub pending_activated: Option<bool>,
}

impl Window {
    /// `done`：待生效的整份生效。从没激活变成激活时用 `order` 取一个新的序号；
    /// 激活着又报一次激活（最大化之类改了别的状态）不算重新激活，序号不变。
    pub fn apply(&mut self, order: impl FnOnce() -> u64) {
        if let Some(app) = self.pending_app.take() {
            self.app = Some(app);
        }
        match self.pending_activated.take() {
            Some(true) if self.activated.is_none() => self.activated = Some(order()),
            Some(false) => self.activated = None,
            _ => {}
        }
    }
}
