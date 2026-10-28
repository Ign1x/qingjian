//! 托盘线程退出后的唤醒 socket：收完排队的点击后不再加入主循环的 poll。

use std::os::unix::net::UnixStream;
use std::sync::mpsc;

use tokio::sync::watch;

use crate::session::ModeTarget;
use crate::tray::Tray;

#[test]
fn a_stopped_worker_removes_the_wakeup_fd() {
    let (modes, _receiver) = watch::channel(None);
    let (sender, clicks) = mpsc::channel();
    let (wake, waker) = UnixStream::pair().unwrap();
    wake.set_nonblocking(true).unwrap();
    let mut tray = Tray::new(modes, clicks, Some(wake));
    assert!(tray.take().is_empty());
    assert!(tray.fd().is_some());
    sender.send(ModeTarget::English(true)).unwrap();
    drop(waker);
    assert_eq!(tray.take(), [ModeTarget::English(true)]);
    assert!(tray.fd().is_none());
    assert!(tray.take().is_empty());
}
