//! Linux XDG 配置、用户数据、日志和随包产品资源定位。
use std::path::{Path, PathBuf};

fn xdg(name: &str, fallback: &str) -> PathBuf {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(fallback)))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("qingjian")
}
pub fn user_dir() -> PathBuf {
    xdg("XDG_DATA_HOME", ".local/share")
}
pub fn config_path() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config").join("config.toml")
}
pub fn log_dir() -> PathBuf {
    xdg("XDG_STATE_HOME", ".local/state").join("logs")
}
pub fn resource_root() -> Option<PathBuf> {
    if let Some(root) = std::env::var_os("QINGJIAN_RESOURCES") {
        return Some(root.into());
    }
    let executable = std::env::current_exe().ok()?;
    let prefix = executable.parent()?.parent()?;
    let installed = prefix.join("share/qingjian/resources");
    if installed.join("assets").is_dir() || installed.join("data").is_dir() {
        return Some(installed);
    }
    qingjian_platform::resources::bundled_root()
}
pub fn existing(path: PathBuf) -> Option<PathBuf> {
    path.is_file().then_some(path)
}
pub fn generated(root: &Path, name: &str) -> Option<PathBuf> {
    existing(root.join("data/generated").join(name))
}
pub fn asset(root: &Path, name: &str) -> Option<PathBuf> {
    existing(root.join("assets").join(name))
}

/// 知意用系统或手动安装的 CPU 运行库；不下载、不探测 GPU。
pub fn onnx_runtime(user: &Path, root: &Path) -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("ORT_DYLIB_PATH") {
        // 显式路径错误时也不偷换成别处的运行库，方便定位版本问题。
        return existing(path.into());
    }
    let dirs = [user.join("models/onnxruntime/lib"), root.join("lib")]
        .into_iter()
        .chain(
            std::env::var_os("LD_LIBRARY_PATH")
                .map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
                .unwrap_or_default()
                .into_iter()
                .filter(|path| path.is_absolute()),
        )
        .chain(
            [
                "/usr/local/lib",
                "/usr/lib",
                "/usr/lib64",
                "/usr/lib/x86_64-linux-gnu",
                "/usr/lib/aarch64-linux-gnu",
            ]
            .map(PathBuf::from),
        );
    find_runtime(dirs)
}

fn find_runtime(dirs: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    dirs.into_iter().find_map(|dir| {
        ["libonnxruntime.so", "libonnxruntime.so.1"]
            .into_iter()
            .find_map(|name| existing(dir.join(name)))
    })
}

#[cfg(test)]
mod tests {
    use super::find_runtime;

    #[test]
    fn runtime_search_preserves_directory_priority_and_accepts_soname() {
        let root = std::env::temp_dir().join(format!("qj-runtime-paths-{}", std::process::id()));
        let user = root.join("user");
        let system = root.join("system");
        std::fs::create_dir_all(&user).unwrap();
        std::fs::create_dir_all(&system).unwrap();
        let soname = user.join("libonnxruntime.so.1");
        std::fs::write(&soname, b"").unwrap();
        std::fs::write(system.join("libonnxruntime.so"), b"").unwrap();
        assert_eq!(find_runtime([user, system]), Some(soname));
        std::fs::remove_dir_all(root).unwrap();
    }
}
