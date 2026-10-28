//! 私有总线上的假托盘宿主，记录注册。

use std::sync::{Arc, Mutex};

/// 假托盘宿主：记下登记来的总线名。
pub(super) struct Watcher {
    pub(super) registered: Arc<Mutex<Vec<String>>>,
}

#[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
impl Watcher {
    fn register_status_notifier_item(&self, service: &str) {
        self.registered.lock().unwrap().push(service.to_owned());
    }
}
