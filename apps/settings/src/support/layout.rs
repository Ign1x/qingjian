//! 设置程序与前端装在同一前缀下（install.sh、deb / rpm 与 Nix 包都是），所以看前缀里的文件判断装了哪个版本。
//!
//! `current` 的前提是设置程序自己在 `<prefix>/bin` 下；cargo 的 `target/debug` 不满足，
//! 开发目录里判不出来，交给 `Installed` 兜底。
use std::path::{Path, PathBuf};

/// 一份安装的目录布局：前缀，以及用户的 data / config 目录。
pub struct Layout {
    prefix: PathBuf,
    data: PathBuf,
    config: PathBuf,
}

impl Layout {
    /// 装好的位置；设置程序不在 `bin` 下（开发目录）时为 `None`。
    pub fn current() -> Option<Self> {
        let exe = std::env::current_exe().ok()?;
        let bin = exe.parent()?;
        if bin.file_name()? != "bin" {
            return None;
        }
        let prefix = bin.parent()?.to_path_buf();
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let data = xdg(home.as_deref(), "XDG_DATA_HOME", ".local/share")?;
        let config = xdg(home.as_deref(), "XDG_CONFIG_HOME", ".config")?;
        Some(Self {
            prefix,
            data,
            config,
        })
    }

    /// Wayland 原生版前端的可执行文件。
    pub fn wayland_frontend(&self) -> PathBuf {
        self.prefix.join("bin/qingjian-linux-wayland")
    }

    /// 各安装方式留下的 fcitx（含 ibus）痕迹，任一存在就算装了。
    pub fn fcitx_marks(&self) -> Vec<PathBuf> {
        vec![
            // install.sh 的用户目录布局与 deb / rpm 的 /usr 布局
            self.prefix.join("lib/fcitx5/qingjian.so"),
            self.prefix.join("lib/fcitx/fcitx-qingjian.so"),
            // IBus 前端也走框架面板
            self.prefix.join("bin/qingjian-linux-ibus"),
            self.prefix.join("share/fcitx5/addon/qingjian.conf"),
            self.prefix.join("share/fcitx/addon/fcitx-qingjian.conf"),
            // 框架自己的用户目录，install.sh 与包管理器都可能只写这里
            self.data.join("fcitx5/addon/qingjian.conf"),
            self.data.join("ibus/component/qingjian.xml"),
            self.config.join("fcitx/addon/fcitx-qingjian.conf"),
        ]
    }

    #[cfg(test)]
    pub fn at(prefix: PathBuf, data: PathBuf, config: PathBuf) -> Self {
        Self {
            prefix,
            data,
            config,
        }
    }
}

/// XDG 目录：只认绝对路径，没设或设成相对路径时回落到 `$HOME` 下的默认位置。
fn xdg(home: Option<&Path>, var: &str, fallback: &str) -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os(var).map(PathBuf::from)
        && dir.is_absolute()
    {
        return Some(dir);
    }
    home.map(|home| home.join(fallback))
}
