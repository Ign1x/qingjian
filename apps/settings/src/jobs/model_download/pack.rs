//! 实验模型的下载版本与用户目录；无 Metal 的平台用 int8 ONNX 版知意。

use std::path::{Path, PathBuf};

use qingjian_platform::LocalModel;

/// 一个能在「实验功能」里下载的模型：从模型仓库的哪个 Release 取，解到用户目录的哪里。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pack {
    pub model: LocalModel,

    /// 模型仓库（qingjian-team/qingjian-models）的标签。主版本写死在这里：换了主版本，旧输入法读不了。
    pub tag: &'static str,

    /// 压缩包里那一层目录。
    archive_dir: &'static str,

    /// 解到用户目录下的哪里，与输入法读的位置一致。
    install_dir: &'static str,

    /// 有这个文件就算装好了。
    marker: &'static str,

    /// 卡片上写的大小。
    pub size: &'static str,
}

impl Pack {
    /// macOS 用 safetensors 版（candle 跑）。
    #[cfg(target_os = "macos")]
    const ZHIYI: Self = Self {
        model: LocalModel::Zhiyi,
        tag: "zhiyi-v1.0.0",
        archive_dir: "zhiyi",
        install_dir: "models/zhiyi",
        marker: "model.safetensors",
        size: "154 MB",
    };

    /// Windows 与 Linux 用 int8 ONNX 版（ONNX Runtime 跑）。
    #[cfg(not(target_os = "macos"))]
    const ZHIYI: Self = Self {
        model: LocalModel::Zhiyi,
        tag: "zhiyi-onnx-v1.0.0",
        archive_dir: "zhiyi-onnx",
        install_dir: "models/zhiyi-onnx",
        marker: "laya.int8g.onnx",
        size: "70 MB",
    };

    pub const ALL: [Self; 2] = [
        Self::ZHIYI,
        Self {
            model: LocalModel::Zhiwei,
            tag: "hanzhang-zhiwei-v1.0.0",
            archive_dir: "hanzhang/zhiwei",
            install_dir: "models/hanzhang-zhiwei",
            marker: "hanzhang-zhiwei-small.qjm",
            size: "52 MB",
        },
    ];

    pub fn of(model: LocalModel) -> Option<Self> {
        Self::ALL.into_iter().find(|pack| pack.model == model)
    }

    pub fn install_path(self, user_dir: &Path) -> PathBuf {
        user_dir.join(self.install_dir)
    }

    pub fn archive_path(self, extracted: &Path) -> PathBuf {
        extracted.join(self.archive_dir)
    }

    /// 删掉装好的整个模型目录。
    pub fn remove(self) -> Result<(), String> {
        let dir = crate::system::paths::user_dir().ok_or("找不到数据目录")?;
        std::fs::remove_dir_all(self.install_path(&dir)).map_err(|error| format!("删不掉：{error}"))
    }

    pub fn installed(self) -> bool {
        crate::system::paths::user_dir()
            .is_some_and(|dir| self.install_path(&dir).join(self.marker).is_file())
    }
}

#[cfg(all(test, not(target_os = "macos")))]
mod tests {
    use super::Pack;
    use qingjian_platform::LocalModel;
    use std::path::Path;

    #[test]
    fn cpu_platforms_download_the_onnx_pack() {
        let pack = Pack::of(LocalModel::Zhiyi).unwrap();
        assert_eq!(pack.tag, "zhiyi-onnx-v1.0.0");
        assert_eq!(
            pack.install_path(Path::new("data")),
            Path::new("data/models/zhiyi-onnx")
        );
        assert_eq!(pack.marker, "laya.int8g.onnx");
        assert_eq!(pack.size, "70 MB");
    }
}
