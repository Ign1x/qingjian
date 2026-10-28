//! 一次模型加载需要的文件。

use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq)]
pub enum ModelFiles {
    /// 含章的 `.qjm` 文件或三件套目录。
    Sentence(PathBuf),

    /// 知意 int8 ONNX 的模型目录与运行库，由壳定位运行库。
    Zhiyi { dir: PathBuf, runtime: PathBuf },
}
