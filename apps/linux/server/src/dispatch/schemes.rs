//! 随包的方案目录：形码、双拼与辅码方案都在这里与用户数据目录的 `schemes/` 里找，与 Windows Server 的 `dispatch/schemes.rs` 对齐。

use std::path::{Path, PathBuf};

/// 随包的方案目录。走 `assets/` 而不是 `data/`：码表随 git 跟踪，装机与开发布局里是同一个相对路径（理由同 Windows Server）。
const BUNDLED_SCHEMES: &str = "assets/schemes";

/// 随包的方案目录；不在为 `None`。
pub fn find_bundled_schemes(root: &Path) -> Option<PathBuf> {
    let dir = root.join(BUNDLED_SCHEMES);
    dir.is_dir().then_some(dir)
}
