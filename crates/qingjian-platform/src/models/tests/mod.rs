//! 模型选择、缺文件回退与后台加载失败；全部用临时目录，不下载模型。
mod loader;
mod resolve;

use std::path::{Path, PathBuf};

fn directory(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("qingjian-models-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn touch(root: &Path, name: &str) -> PathBuf {
    let path = root.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"").unwrap();
    path
}
