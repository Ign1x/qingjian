/// 设置程序跑在哪个平台上。Linux 分两个版本，装的是哪个由 `Installed` 探出来，看系统判断不了。
// 四个变体都参与表与 `status` 判据，但单个目标上的变体只出现在 match 分支里，会被当成没构造过。
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    MacOs,
    Windows,
    /// Linux + Fcitx5 / Fcitx4：候选窗由框架面板画，键位提示与矩阵画不了。
    LinuxFcitx,
    /// Linux + Wayland 原生版前端：自己当 input-method-v2 客户端，候选窗自绘。
    LinuxWayland,
}

impl Platform {
    #[cfg(test)]
    pub const ALL: [Self; 4] = [
        Self::MacOs,
        Self::Windows,
        Self::LinuxFcitx,
        Self::LinuxWayland,
    ];

    #[cfg(test)]
    pub fn label(self) -> &'static str {
        match self {
            Self::MacOs => "macOS",
            Self::Windows => "Windows",
            Self::LinuxFcitx => "Linux fcitx 版",
            Self::LinuxWayland => "Linux Wayland 原生版",
        }
    }
}
