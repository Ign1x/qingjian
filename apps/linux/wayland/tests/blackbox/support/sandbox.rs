//! 一次测试的临时目录：合成器与 Server 的 socket、XDG 各目录与 HOME 都在里面，测完整个删掉。

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    pub fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        // 名字要短：socket 路径整个不能超过 108 字节
        let root = std::env::temp_dir().join(format!(
            "qingjian-wayland-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("建临时目录");
        // 当 XDG_RUNTIME_DIR 用，按规范只给自己
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
            .expect("临时目录改成 0700");
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
