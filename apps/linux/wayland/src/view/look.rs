//! 候选窗的外观设置：配置里与画候选窗有关的几项（主题、字体、字号、动画）。
//! 每秒按戳查一次配置文件，变了才重读；读出来与在用的一样就不重建主题与字体。

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use qingjian_platform::{Config, ConfigError, GeneralConfig};
use qingjian_render::{TextSizes, Theme, ThemeLibrary};

#[derive(Debug, Clone, PartialEq)]
pub struct Look {
    /// 主题 id（`[general] theme`，旧写法已换成内置主题）。
    pub theme_id: String,

    /// 界面字体的字族名（`[general] font`）；空为系统字体。
    pub font: String,

    /// 候选字、译文字号，盖过主题的。
    pub sizes: TextSizes,

    /// 过渡与循环动画（`[general] animations`）。
    pub animations: bool,
}

impl Look {
    pub fn new(general: &GeneralConfig) -> Self {
        Self {
            theme_id: general.theme_id().to_owned(),
            font: general.font.trim().to_owned(),
            sizes: TextSizes {
                candidate: general.candidate_font_size.get(),
                annotation: general.annotation_font_size.get(),
            },
            animations: general.animations,
        }
    }

    /// 读配置文件（只读不写）；文件不在是缺省配置。
    pub fn read(path: &Path) -> Result<Self, ConfigError> {
        Config::load(path).map(|config| Self::new(&config.general))
    }

    /// 配置文件的戳：只 stat 不读内容，文件改了它就变；文件不在也是一个固定的值。
    pub fn stamp(path: &Path) -> u64 {
        let mut hasher = DefaultHasher::new();
        if let Ok(meta) = std::fs::metadata(path) {
            // 换了文件（原子保存、home-manager 换链接指向）时 inode 变；nix store 里文件的修改时间都是 1970
            meta.ino().hash(&mut hasher);
            meta.len().hash(&mut hasher);
            meta.modified().ok().hash(&mut hasher);
        }
        hasher.finish()
    }

    /// 从主题目录（`None` 只有内置主题）取这份外观的主题，浅色那一份，套上字号与动画开关。
    /// 没有这个主题时退回缺省主题（主题库记警告）。
    pub fn theme(&self, themes_dir: Option<&Path>) -> Theme {
        ThemeLibrary::load(themes_dir)
            .resolve(&self.theme_id, false)
            .with_text_sizes(self.sizes)
            .with_animations(self.animations)
    }
}
