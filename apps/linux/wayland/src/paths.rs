//! 配置、主题与日志放哪：与 Server（`apps/linux/server/src/paths.rs`）、设置程序同一套 XDG 约定。

use std::path::PathBuf;

fn xdg(name: &str, fallback: &str) -> PathBuf {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(fallback)))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("qingjian")
}

pub fn config_path() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config").join("config.toml")
}

/// 用户主题目录，一个主题一个子目录。
pub fn themes_dir() -> PathBuf {
    xdg("XDG_DATA_HOME", ".local/share").join("themes")
}

pub fn log_dir() -> PathBuf {
    xdg("XDG_STATE_HOME", ".local/state").join("logs")
}
