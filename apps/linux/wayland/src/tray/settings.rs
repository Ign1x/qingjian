//! 托盘菜单的「设置…」：拉起设置程序。

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const PROGRAM: &str = "qingjian-settings";

/// 设置程序在 Linux 上不管单开：连点两下会开出两个窗口、各自拿着一份旧配置，所以开着时不再开第二个。
#[derive(Debug, Default)]
pub struct Launcher {
    /// 拉起的那个还没退出。
    running: Arc<AtomicBool>,
}

impl Launcher {
    /// 与前端同目录的 `qingjian-settings`（安装脚本与发行包都把两个装在一起），没有就交给 PATH 找。
    pub fn open(&self) {
        if self.running.swap(true, Ordering::AcqRel) {
            tracing::info!("设置程序已经开着");
            return;
        }
        let program = beside_frontend().unwrap_or_else(|| PathBuf::from(PROGRAM));
        let spawned = Command::new(&program)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(error) => {
                tracing::warn!(%error, program = %program.display(), "设置程序起不来");
                self.running.store(false, Ordering::Release);
                return;
            }
        };
        tracing::info!("打开设置程序");
        // 等它退出：收掉子进程，也好知道下次能再开
        let running = Arc::clone(&self.running);
        let waiting = std::thread::Builder::new()
            .name("qingjian-settings-wait".to_owned())
            .spawn(move || {
                let _ = child.wait();
                running.store(false, Ordering::Release);
            });
        if let Err(error) = waiting {
            tracing::warn!(%error, "等不了设置程序退出，之后托盘菜单里的「设置…」可能打不开");
        }
    }
}

fn beside_frontend() -> Option<PathBuf> {
    let path = std::env::current_exe().ok()?.with_file_name(PROGRAM);
    path.is_file().then_some(path)
}
