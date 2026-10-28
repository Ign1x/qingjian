//! 主线程这一头的托盘：把模式交给托盘线程，收托盘上点出来的切换。

use std::io::{ErrorKind, Read};
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixStream;
use std::sync::mpsc;

use qingjian_platform::protocol::linux::InputMode;
use tokio::sync::watch;

use crate::session::ModeTarget;

pub struct Tray {
    /// 现在的模式；托盘线程跟着它换图标与菜单的勾。`None` 是还不知道（没连上 Server、还没进过输入框）。
    modes: watch::Sender<Option<InputMode>>,

    /// 托盘上点出来的切换，按先后。
    clicks: mpsc::Receiver<ModeTarget>,

    /// 托盘线程排了切换就写一个字节；主循环把它和合成器的连接一起 `poll`。建不起来时是 `None`（也就没有托盘线程）。
    wake: Option<UnixStream>,
}

impl Tray {
    pub(super) fn new(
        modes: watch::Sender<Option<InputMode>>,
        clicks: mpsc::Receiver<ModeTarget>,
        wake: Option<UnixStream>,
    ) -> Self {
        Self {
            modes,
            clicks,
            wake,
        }
    }

    /// 换上现在的模式，变了托盘线程才换图标。
    pub fn show(&self, mode: Option<&InputMode>) {
        self.modes.send_if_modified(|shown| {
            if shown.as_ref() == mode {
                return false;
            }
            *shown = mode.cloned();
            true
        });
    }

    /// 主循环要一起等的那个 fd：可读就是托盘上点了东西。
    pub fn fd(&self) -> Option<BorrowedFd<'_>> {
        self.wake.as_ref().map(AsFd::as_fd)
    }

    /// 取走托盘上点出来的切换，顺带读空唤醒的字节。先读字节再取：之后才排进来的那一下自己会再写一个字节。
    pub fn take(&mut self) -> Vec<ModeTarget> {
        if let Some(wake) = self.wake.as_mut() {
            let mut bytes = [0; 64];
            loop {
                match wake.read(&mut bytes) {
                    Ok(0) => break,
                    Ok(_) => continue,
                    Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        return self.clicks.try_iter().collect();
                    }
                    Err(_) => break,
                }
            }
            // 后台线程退出后 socket 会持续报 POLLHUP；移出 poll，避免没有会话总线时主循环空转。
            self.wake = None;
        }
        self.clicks.try_iter().collect()
    }
}
