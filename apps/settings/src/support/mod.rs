//! 只在部分平台显示的设置项：各平台的输入法读不读它，设置程序就按这张表显示或隐藏。
//! 新加的设置项若只做了部分平台，在这里登记其余平台是「不适用」还是「待实现」。
//! Linux 分 fcitx 版与 Wayland 原生版两列，实际显示哪几列由 `Installed` 探出来。

mod platform;
mod status;

#[cfg(not(any(target_os = "macos", windows)))]
mod installed;
#[cfg(not(any(target_os = "macos", windows)))]
mod layout;

pub use platform::Platform;
pub use status::Status;

use std::borrow::Cow;

#[cfg(test)]
use crate::page_id::PageId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feature {
    /// 输入页的辅码与码表。
    AuxCode,
    /// 形码，以及用户目录 `schemes/` 里的形码与双拼方案。
    Schemes,
    /// 中英文切换键。
    SwitchKeys,
    /// 切换键里的单击 Ctrl 与 Ctrl + Alt + Space：macOS 只认单击 Shift。
    SwitchKeyCombos,
    /// 切换键里的「Ctrl + Space（系统）」：改的是 Windows 的系统热键。
    SystemCtrlSpace,
    DefaultEnglish,
    RememberModePerApp,
    /// 英文模式的全半角标点。
    EnglishPunctuation,
    AutoSpace,
    StatusBar,
    /// 横排时 ↑ / ↓ 展开成多行。
    HorizontalGrid,
    ExtraPageKeys,
    HighlightKeys,
    TogglePunctuation,
    /// 简体与上次用的繁体之间切换输出字形。
    ToggleTraditional,
    SystemTextReplacements,
    /// 统计页的云服务用量，测试连接也记进去。
    ApiUsage,
    /// Windows 的手动云联想与请求节奏设置。
    CloudRequestControls,
    /// 上屏的整句翻译后记进生词本（随释义兜底）。
    VocabularyTranslation,
    /// 双拼键位提示：候选窗旁画键盘。
    KeyHint,
    /// 拼音行按高亮候选显示带调拼音。
    ShowTones,
    /// 「实验功能」页：下载、启用知意与含章·知微。
    ExperimentalModels,
    /// 中文组句中的小键盘数字：选候选还是进组句。
    NumpadDigit,
}

impl Feature {
    #[cfg(test)]
    pub const ALL: [Self; 23] = [
        Self::AuxCode,
        Self::Schemes,
        Self::SwitchKeys,
        Self::SwitchKeyCombos,
        Self::SystemCtrlSpace,
        Self::DefaultEnglish,
        Self::RememberModePerApp,
        Self::EnglishPunctuation,
        Self::AutoSpace,
        Self::StatusBar,
        Self::HorizontalGrid,
        Self::ExtraPageKeys,
        Self::HighlightKeys,
        Self::TogglePunctuation,
        Self::ToggleTraditional,
        Self::SystemTextReplacements,
        Self::ApiUsage,
        Self::CloudRequestControls,
        Self::VocabularyTranslation,
        Self::KeyHint,
        Self::ShowTones,
        Self::ExperimentalModels,
        Self::NumpadDigit,
    ];

    /// 当前装的输入法里有一个版本显示这一项。
    pub fn here(self) -> bool {
        host()
            .iter()
            .any(|platform| self.status(*platform) == Status::Supported)
    }

    pub fn status(self, platform: Platform) -> Status {
        let [macos, windows, fcitx, wayland] = self.table();
        match platform {
            Platform::MacOs => macos,
            Platform::Windows => windows,
            Platform::LinuxFcitx => fcitx,
            Platform::LinuxWayland => wayland,
        }
    }

    /// macOS、Windows、Linux fcitx 版、Linux Wayland 原生版依次。
    fn table(self) -> [Status; 4] {
        use Status::{NotApplicable as No, Planned as Todo, Supported as Yes};
        match self {
            Self::AuxCode => [Yes, Yes, Yes, Yes],
            Self::Schemes => [Yes, Yes, Yes, Yes],
            Self::SwitchKeys => [Yes, Yes, Yes, Yes],
            // macOS 只做了单击 Shift，Caps Lock 仍是系统那一套
            Self::SwitchKeyCombos => [No, Yes, Yes, Yes],
            Self::SystemCtrlSpace => [No, Yes, No, No],
            Self::DefaultEnglish => [Todo, Yes, Yes, Yes],
            Self::RememberModePerApp => [Todo, Yes, Yes, Yes],
            // macOS 英文模式一律半角
            Self::EnglishPunctuation => [No, Yes, Yes, Yes],
            Self::AutoSpace => [Yes, Yes, Yes, Yes],
            // 悬浮状态条只有 Windows；两个 Linux 版本都用框架托盘 / 自绘托盘
            Self::StatusBar => [No, Yes, No, No],
            // 矩阵只有自绘候选窗画得了：fcitx 面板不适用，Wayland 原生版支持
            Self::HorizontalGrid => [Yes, Yes, No, Yes],
            Self::ExtraPageKeys => [Yes, Yes, Yes, Yes],
            Self::HighlightKeys => [Yes, Yes, Yes, Yes],
            Self::TogglePunctuation => [Yes, Yes, Yes, Yes],
            Self::ToggleTraditional => [Todo, Yes, Yes, Yes],
            // 系统的文本替换只有 macOS 有
            Self::SystemTextReplacements => [Yes, No, No, No],
            Self::ApiUsage => [Yes, Yes, Yes, Yes],
            Self::CloudRequestControls => [Yes, Yes, Yes, Yes],
            Self::VocabularyTranslation => [Yes, Yes, Yes, Yes],
            // 键位提示同矩阵：fcitx 不适用，Wayland 原生版支持
            Self::KeyHint => [Yes, Yes, No, Yes],
            Self::ShowTones => [Yes, Yes, Yes, Yes],
            Self::ExperimentalModels => [Yes, Yes, Yes, Yes],
            Self::NumpadDigit => [Todo, Yes, Yes, Yes],
        }
    }

    /// Linux 两列里正好一个支持时返回那一列；都支持或都不支持（含「待实现」）时为 `None`。
    fn linux_only(self) -> Option<Platform> {
        let [_, _, fcitx, wayland] = self.table();
        match (fcitx == Status::Supported, wayland == Status::Supported) {
            (true, false) => Some(Platform::LinuxFcitx),
            (false, true) => Some(Platform::LinuxWayland),
            _ => None,
        }
    }

    /// Linux 上这一项只有一个版本支持时，行说明后面该括注哪个版本。
    pub fn linux_note(self) -> Option<&'static str> {
        if !cfg!(target_os = "linux") {
            return None;
        }
        match self.linux_only()? {
            Platform::LinuxFcitx => Some("（仅 fcitx 版）"),
            Platform::LinuxWayland => Some("（仅 Wayland 原生版）"),
            // linux_only 只返回 Linux 的两列
            _ => None,
        }
    }

    /// 行说明：有版本括注时拼在后面，没有就原样借用。
    pub fn describe<'a>(self, text: &'a str) -> Cow<'a, str> {
        match self.linux_note() {
            Some(note) => Cow::Owned(format!("{text}{note}")),
            None => Cow::Borrowed(text),
        }
    }

    /// 在 `docs/plan/todo.md` 里的叫法。
    #[cfg(test)]
    pub fn label(self) -> &'static str {
        match self {
            Self::AuxCode => "辅码",
            Self::Schemes => "形码与自定义方案",
            Self::SwitchKeys => "中英文切换键",
            Self::SwitchKeyCombos => "单击 Ctrl 与 Ctrl + Alt + Space 切换",
            Self::SystemCtrlSpace => "系统的 Ctrl + Space",
            Self::DefaultEnglish => "默认英文模式",
            Self::RememberModePerApp => "按应用记住中英模式",
            Self::EnglishPunctuation => "英文标点全半角",
            Self::AutoSpace => "中文与英文、数字之间自动加空格",
            Self::StatusBar => "悬浮状态条",
            Self::HorizontalGrid => "横排时 ↑ / ↓ 展开成多行",
            Self::ExtraPageKeys => "额外翻页键",
            Self::HighlightKeys => "高亮移动键",
            Self::TogglePunctuation => "Ctrl + . 切换全半角标点",
            Self::ToggleTraditional => "Ctrl + Shift + F 切换简繁输出",
            Self::SystemTextReplacements => "也用系统的文本替换",
            Self::ApiUsage => "云服务用量统计",
            Self::CloudRequestControls => "按需云联想与请求节奏",
            Self::VocabularyTranslation => "生词本整句翻译",
            Self::KeyHint => "双拼键位提示",
            Self::ShowTones => "拼音行显示声调",
            Self::ExperimentalModels => "实验功能（知意、含章·知微）",
            Self::NumpadDigit => "小键盘数字",
        }
    }

    /// 这一项建出来的、带 ⓘ 链接的行（页、行名）；不显示时这些行也没有。
    #[cfg(test)]
    pub fn rows(self) -> &'static [(PageId, &'static str)] {
        match self {
            Self::AuxCode => &[
                (PageId::Input, "启用"),
                (PageId::Input, "触发键"),
                (PageId::Input, "导入码表"),
            ],
            Self::Schemes => &[(PageId::Input, "形码")],
            Self::SwitchKeys => &[(PageId::Language, "切换键")],
            Self::DefaultEnglish => &[(PageId::Language, "默认英文模式")],
            Self::RememberModePerApp => &[(PageId::Language, "按应用记住中英模式")],
            Self::AutoSpace => &[(PageId::Language, "中文与英文、数字之间自动加空格")],
            Self::ExtraPageKeys => &[(PageId::Shortcuts, "额外翻页键")],
            Self::HighlightKeys => &[(PageId::Shortcuts, "高亮移动键")],
            Self::TogglePunctuation => &[(PageId::Shortcuts, "Ctrl + . 切换全半角标点")],
            Self::ToggleTraditional => &[(PageId::Shortcuts, "Ctrl + Shift + F 切换简繁输出")],
            Self::SystemTextReplacements => &[(PageId::Phrases, "也用系统的文本替换")],
            Self::CloudRequestControls => &[
                (PageId::Cloud, "自动联想"),
                (PageId::Cloud, "停键等待"),
                (PageId::Cloud, "最短请求间隔"),
                (PageId::Shortcuts, "按需 AI 联想"),
            ],
            Self::VocabularyTranslation => &[(PageId::Cloud, "生词本整句翻译")],
            Self::KeyHint => &[(PageId::Input, "键位提示")],
            Self::ShowTones => &[(PageId::Candidates, "拼音行显示声调")],
            Self::NumpadDigit => &[(PageId::Candidates, "小键盘数字")],
            Self::SwitchKeyCombos
            | Self::SystemCtrlSpace
            | Self::EnglishPunctuation
            | Self::StatusBar
            | Self::HorizontalGrid
            | Self::ApiUsage
            | Self::ExperimentalModels => &[],
        }
    }
}

/// 当前这一份设置程序该按哪些平台判断：macOS / Windows 只有自己一列，Linux 看装了哪个版本。
#[cfg(target_os = "macos")]
fn host() -> &'static [Platform] {
    &[Platform::MacOs]
}

#[cfg(windows)]
fn host() -> &'static [Platform] {
    &[Platform::Windows]
}

#[cfg(not(any(target_os = "macos", windows)))]
fn host() -> &'static [Platform] {
    installed::Installed::current().platforms()
}

/// 某页的某行只在别的平台建出来。
#[cfg(test)]
pub fn elsewhere(page: PageId, row: &str) -> bool {
    Feature::ALL
        .iter()
        .any(|feature| !feature.here() && feature.rows().contains(&(page, row)))
}

#[cfg(test)]
mod tests {
    use super::{Feature, Platform, Status};

    /// `docs/plan/todo.md` 里「设置项的平台缺口」下的条目与表里的「待实现」一一对应。
    #[test]
    fn planned_gaps_are_listed_in_todo() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/plan/todo.md");
        let todo = std::fs::read_to_string(path).unwrap();
        let listed: Vec<&str> = todo
            .lines()
            .skip_while(|line| !line.contains("设置项的平台缺口"))
            .skip(1)
            .take_while(|line| line.starts_with("  "))
            .collect();
        let expected: Vec<String> = Feature::ALL
            .iter()
            .filter_map(|feature| {
                let platforms: Vec<&str> = Platform::ALL
                    .iter()
                    .filter(|platform| feature.status(**platform) == Status::Planned)
                    .map(|platform| platform.label())
                    .collect();
                (!platforms.is_empty())
                    .then(|| format!("  - [ ] {}：{}", feature.label(), platforms.join("、")))
            })
            .collect();
        assert_eq!(
            listed, expected,
            "todo.md 的「设置项的平台缺口」要照 support 表的「待实现」逐条列"
        );
    }

    /// 每一项至少一个平台在用，不然该删掉。
    #[test]
    fn every_feature_is_supported_somewhere() {
        for feature in Feature::ALL {
            assert!(
                Platform::ALL
                    .iter()
                    .any(|platform| feature.status(*platform) == Status::Supported),
                "{} 哪个平台都没有",
                feature.label()
            );
        }
    }

    /// 只有一个 Linux 版本支持的项要在行说明里括注版本，其余项不许有括注。
    #[test]
    fn single_edition_rows_are_marked() {
        for feature in Feature::ALL {
            let marked = matches!(feature, Feature::HorizontalGrid | Feature::KeyHint);
            let expected = marked.then_some(Platform::LinuxWayland);
            assert_eq!(
                feature.linux_only(),
                expected,
                "{} 的单版本括注不对",
                feature.label()
            );
        }
    }
}
