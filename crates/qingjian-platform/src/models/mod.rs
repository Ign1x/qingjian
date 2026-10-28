//! 本地模型的目录解析与后台加载：含章整句重排、知意 CPU 两线程选词，壳只负责接到 Engine。

mod dirs;
mod files;
mod loaded;
mod loader;
mod model;

pub use dirs::{ModelDirs, find_model};
pub use files::ModelFiles;
pub use loaded::Loaded;
pub use loader::ModelLoader;
pub use model::LoadedModel;

#[cfg(test)]
mod tests;
