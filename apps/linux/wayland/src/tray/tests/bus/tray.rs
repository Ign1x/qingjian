//! 私有总线上的被测托盘及主线程一端。

use crate::session::ModeTarget;
use crate::testing::Bus;
use crate::tray::{clicks::Clicks, serve};
use qingjian_platform::protocol::linux::InputMode;
use std::io::Read;
use std::os::unix::net::UnixStream;
use std::sync::{Arc, mpsc};
use tokio::sync::watch;

/// 被测的托盘那一头：主线程该拿着的几样。
pub(super) struct Tray {
    pub(super) modes: watch::Sender<Option<InputMode>>,

    clicks: mpsc::Receiver<ModeTarget>,

    wake: UnixStream,
}

impl Tray {
    /// 在这条总线上起托盘（像 `spawn` 那样一直跑），模式起初不知道。
    pub(super) async fn start(bus: &Bus) -> (Self, tokio::task::JoinHandle<()>) {
        let (modes, receiver) = watch::channel(None);
        let (sender, clicks) = mpsc::channel();
        let (wake, waker) = UnixStream::pair().unwrap();
        wake.set_nonblocking(true).unwrap();
        let connection = bus.connect().await;
        let serving = tokio::spawn(async move {
            serve(&connection, receiver, Clicks::new(sender, Arc::new(waker)))
                .await
                .unwrap();
        });
        let tray = Self {
            modes,
            clicks,
            wake,
        };
        (tray, serving)
    }

    /// 托盘线程排过来的切换，连同叫醒主循环的那个字节。
    pub(super) fn clicked(&self) -> Option<ModeTarget> {
        let mut bytes = [0; 8];
        let woken = matches!((&self.wake).read(&mut bytes), Ok(read) if read > 0);
        let target = self.clicks.try_recv().ok();
        assert_eq!(woken, target.is_some(), "排了切换就要叫醒主循环");
        target
    }
}
