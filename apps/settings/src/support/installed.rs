//! Linux 上装的是哪个版本：Wayland 原生版前端在，或框架（fcitx5 / fcitx4 / ibus）的文件在。
//!
//! 两样都没找到时按「两个都装了」处理——开发目录、别家的打包（把文件挪到别处）判不出来，
//! 判错成「没装」会把该显示的设置藏起来，宁可多显示。
use std::path::Path;
use std::sync::LazyLock;

use super::layout::Layout;
use super::platform::Platform;

/// `platforms` 返回的列；引用借不出去临时值，所以摊在模块级。
const FCITX: [Platform; 1] = [Platform::LinuxFcitx];
const WAYLAND: [Platform; 1] = [Platform::LinuxWayland];
const BOTH: [Platform; 2] = [Platform::LinuxFcitx, Platform::LinuxWayland];
const NONE: [Platform; 0] = [];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Installed {
    pub fcitx: bool,
    pub wayland: bool,
}

impl Installed {
    /// 只探一次。
    pub fn current() -> Self {
        static CURRENT: LazyLock<Installed> = LazyLock::new(Installed::probe);
        *CURRENT
    }

    /// 设置程序在 `bin` 下才探得出来，否则按两个都装。
    fn probe() -> Self {
        match Layout::current() {
            Some(layout) => Self::from_layout(&layout, |path| path.exists()),
            None => Self::both(),
        }
    }

    /// 纯函数：`exists` 说某个路径在不在，便于单测脱离文件系统。
    pub fn from_layout(layout: &Layout, exists: impl Fn(&Path) -> bool) -> Self {
        let wayland = exists(&layout.wayland_frontend());
        let fcitx = layout.fcitx_marks().iter().any(|mark| exists(mark));
        if fcitx || wayland {
            Self { fcitx, wayland }
        } else {
            Self::both()
        }
    }

    /// 这两个版本各占一列；都没装（探不出来时不会走到）就没有 Linux 列。
    pub fn platforms(self) -> &'static [Platform] {
        match (self.fcitx, self.wayland) {
            (true, true) => &BOTH,
            (true, false) => &FCITX,
            (false, true) => &WAYLAND,
            (false, false) => &NONE,
        }
    }

    fn both() -> Self {
        Self {
            fcitx: true,
            wayland: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Installed, Layout, Platform};
    use std::path::{Path, PathBuf};

    fn layout() -> Layout {
        Layout::at(
            PathBuf::from("/opt/qingjian"),
            PathBuf::from("/home/me/.local/share"),
            PathBuf::from("/home/me/.config"),
        )
    }

    /// 只有 Wayland 前端一个文件。
    #[test]
    fn detects_wayland_only() {
        let layout = layout();
        let frontend = layout.wayland_frontend();
        let installed = Installed::from_layout(&layout, |path| path == frontend);
        assert_eq!(installed.platforms(), &[Platform::LinuxWayland][..]);
    }

    /// 框架的任一痕迹都算 fcitx 版，包括只写了用户目录的那种安装。
    #[test]
    fn detects_each_fcitx_mark() {
        let layout = layout();
        for mark in layout.fcitx_marks() {
            let installed = Installed::from_layout(&layout, |path| path == mark);
            assert_eq!(
                installed.platforms(),
                &[Platform::LinuxFcitx][..],
                "{}",
                mark.display()
            );
        }
    }

    /// 两样都在时两列都算。
    #[test]
    fn detects_both() {
        let layout = layout();
        let frontend = layout.wayland_frontend();
        let installed = Installed::from_layout(&layout, |path| {
            path == frontend
                || path == Path::new("/home/me/.config/fcitx/addon/fcitx-qingjian.conf")
        });
        assert_eq!(
            installed.platforms(),
            &[Platform::LinuxFcitx, Platform::LinuxWayland][..]
        );
    }

    /// 一个都没找到（开发目录、挪过位置的打包）不藏设置。
    #[test]
    fn nothing_found_means_both() {
        let layout = layout();
        let installed = Installed::from_layout(&layout, |_| false);
        assert!(installed.fcitx && installed.wayland);
        assert_eq!(
            installed.platforms(),
            &[Platform::LinuxFcitx, Platform::LinuxWayland][..]
        );
    }

    /// 前端路径都在前缀下，用户目录的痕迹落在 data / config 下。
    #[test]
    fn marks_follow_the_layout() {
        let layout = layout();
        assert_eq!(
            layout.wayland_frontend(),
            Path::new("/opt/qingjian/bin/qingjian-linux-wayland")
        );
        let marks = layout.fcitx_marks();
        assert!(marks.contains(&PathBuf::from("/opt/qingjian/lib/fcitx5/qingjian.so")));
        assert!(marks.contains(&PathBuf::from(
            "/home/me/.local/share/ibus/component/qingjian.xml"
        )));
    }
}
