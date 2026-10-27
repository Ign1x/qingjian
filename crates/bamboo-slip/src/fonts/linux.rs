//! Linux 与 FreeBSD 的字体文件清单：发行版装 Noto 的常见位置；一个都不在时（NixOS 这类）问 fontconfig 要（见 [`super::fontconfig`]）。
//! Debian / Ubuntu 在 `truetype/`、`opentype/` 下，Arch 在 `noto/`、`noto-cjk/`，Fedora 桌面默认装的是可变字体（`google-noto-vf/` 等）。
//! FreeBSD 的字体包装在 `/usr/local/share/fonts`。

use std::path::PathBuf;

use super::fontconfig;

pub(super) fn ui_fonts() -> Vec<PathBuf> {
    let fixed = [
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/google-noto-vf/NotoSans[wght].ttf",
        "/usr/share/fonts/google-noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
        "/usr/local/share/fonts/noto/NotoSans-Regular.ttf",
        "/usr/local/share/fonts/dejavu/DejaVuSans.ttf",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect();
    or_ask(fixed, || fontconfig::best("sans-serif"))
}

pub(super) fn ui_weight_fonts() -> Vec<PathBuf> {
    let fixed = [
        "/usr/share/fonts/truetype/noto/NotoSans-Bold.ttf",
        "/usr/share/fonts/noto/NotoSans-Bold.ttf",
        "/usr/share/fonts/google-noto/NotoSans-Bold.ttf",
        "/usr/local/share/fonts/noto/NotoSans-Bold.ttf",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect();
    or_ask(fixed, || fontconfig::best("sans-serif:style=Bold"))
}

pub(super) fn script_fonts(locale: &str) -> Vec<PathBuf> {
    let fixed = [
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/google-noto-sans-cjk-vf-fonts/NotoSansCJK-VF.ttc",
        "/usr/share/fonts/google-noto-sans-cjk-fonts/NotoSansCJK-Regular.ttc",
        "/usr/local/share/fonts/noto/NotoSansCJKsc-Regular.otf",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect();
    or_ask(fixed, || {
        fontconfig::best_for_lang("sans-serif", &locale.to_ascii_lowercase())
    })
}

pub(super) fn emoji_fonts() -> Vec<PathBuf> {
    let fixed = [
        "/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf",
        "/usr/share/fonts/noto/NotoColorEmoji.ttf",
        "/usr/share/fonts/google-noto-color-emoji-fonts/NotoColorEmoji.ttf",
        "/usr/local/share/fonts/noto/NotoColorEmoji.ttf",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect();
    or_ask(fixed, || fontconfig::best_for_lang("emoji", "und-zsye"))
}

/// 清单里有一个文件在就照用清单；一个都不在才问 fontconfig，把它给的接在后面（问一次要起一个进程）。
fn or_ask(fixed: Vec<PathBuf>, ask: impl FnOnce() -> Option<PathBuf>) -> Vec<PathBuf> {
    if fixed.iter().any(|path| path.is_file()) {
        return fixed;
    }
    fixed.into_iter().chain(ask()).collect()
}
