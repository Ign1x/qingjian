//! 托盘线程这一头：把点出来的切换排给主线程，再叫醒它。

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::sync::{Arc, mpsc};

use crate::session::ModeTarget;

/// 图标与菜单各拿一份。
#[derive(Clone)]
pub struct Clicks {
    sender: mpsc::Sender<ModeTarget>,

    /// 与主线程 [`super::Tray`] 那一头连着的 socket。
    wake: Arc<UnixStream>,
}

impl Clicks {
    pub fn new(sender: mpsc::Sender<ModeTarget>, wake: Arc<UnixStream>) -> Self {
        Self { sender, wake }
    }

    /// 排一个切换给主线程。主线程已经没了（进程在退出）就算了；字节写不进（缓冲满了）也无妨，还没读走的那些照样叫得醒它。
    pub fn send(&self, target: ModeTarget) {
        if self.sender.send(target).is_ok() {
            let _ = (&*self.wake).write(&[1]);
        }
    }
}
