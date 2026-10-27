//! Linux / FreeBSD：问 fontconfig 字体文件在哪。
//!
//! 只在两处用：平台清单里的固定路径一个都不在（NixOS、Guix 这类不把字体放 `/usr/share/fonts` 的系统），
//! 以及按字族名找文件（主题点名的字体与用户指定的界面字体）。走 `fc-match` / `fc-list` 命令，
//! 不链 libfontconfig、不扫字体库；命令不在就当找不到。

use std::path::PathBuf;
use std::process::Command;

/// 某字族所有面的文件路径，去重；没装这个字族或 fontconfig 不在时为空。与 Windows 的同名函数同一签名。
pub fn family_files(family: &str) -> Vec<PathBuf> {
    if family.trim().is_empty() {
        return Vec::new();
    }
    run("fc-list", &["--format", "%{file}\n", &escape(family)])
}

/// fontconfig 心目中这个模式的首选文件，如 `sans-serif`、`sans-serif:weight=bold`。
pub(super) fn best(pattern: &str) -> Option<PathBuf> {
    run("fc-match", &["--format", "%{file}", pattern])
        .into_iter()
        .next()
}

/// 覆盖某个语言（fontconfig 的语言标签：`zh-cn`、`ja`，emoji 是 `und-zsye`）的首选文件。
/// `fc-match` 总会给出一个文件，哪怕它根本不带这个语言的字，所以拿 `fc-list` 按语言列出的再核对一遍。
pub(super) fn best_for_lang(family: &str, lang: &str) -> Option<PathBuf> {
    let covering = run(
        "fc-list",
        &["--format", "%{file}\n", &format!(":lang={lang}")],
    );
    pick(best(&format!("{family}:lang={lang}")), covering)
}

/// 首选的在覆盖列表里就用它，否则取列表里排序后的第一个。
fn pick(preferred: Option<PathBuf>, mut covering: Vec<PathBuf>) -> Option<PathBuf> {
    match preferred {
        Some(path) if covering.contains(&path) => Some(path),
        _ => {
            covering.sort();
            covering.into_iter().next()
        }
    }
}

fn run(program: &str, args: &[&str]) -> Vec<PathBuf> {
    match Command::new(program).args(args).output() {
        Ok(output) if output.status.success() => parse(&output.stdout),
        Ok(output) => {
            tracing::debug!(program, status = %output.status, "fontconfig 没找到字体");
            Vec::new()
        }
        Err(error) => {
            tracing::debug!(program, %error, "fontconfig 命令不可用");
            Vec::new()
        }
    }
}

/// 一行一个路径，只留真有的文件，去重保序（`.ttc` 的每张面各占一行）。
fn parse(stdout: &[u8]) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = Vec::new();
    for line in String::from_utf8_lossy(stdout).lines() {
        let path = PathBuf::from(line.trim());
        if !line.trim().is_empty() && path.is_file() && !files.contains(&path) {
            files.push(path);
        }
    }
    files
}

/// fontconfig 模式里 `\` `-` `:` `,` 有特殊含义，字族名里出现要转义（`Source Han Sans SC-Bold` 这类名字）。
fn escape(family: &str) -> String {
    let mut escaped = String::with_capacity(family.len());
    for character in family.chars() {
        if matches!(character, '\\' | '-' | ':' | ',') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_characters_in_family_names_are_escaped() {
        assert_eq!(escape("Noto Sans CJK SC"), "Noto Sans CJK SC");
        assert_eq!(escape("Foo-Bar: A,B\\"), "Foo\\-Bar\\: A\\,B\\\\");
    }

    #[test]
    fn output_keeps_existing_files_once_in_order() {
        let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let stdout = format!("{0}\n\n{0}\n/no/such/font.ttf\n", here.display());
        assert_eq!(parse(stdout.as_bytes()), vec![here]);
    }

    /// 首选的不带这个语言就换覆盖列表里的；列表空了就是没有。
    #[test]
    fn preferred_file_must_cover_the_language() {
        let a = PathBuf::from("/fonts/a.ttf");
        let b = PathBuf::from("/fonts/b.ttf");
        assert_eq!(
            pick(Some(b.clone()), vec![a.clone(), b.clone()]),
            Some(b.clone())
        );
        assert_eq!(
            pick(Some(PathBuf::from("/fonts/latin.ttf")), vec![b, a.clone()]),
            Some(a)
        );
        assert_eq!(pick(None, Vec::new()), None);
    }

    /// 装了 fontconfig 的机器上应能找到一个界面字体；没装的（精简容器）跳过。
    #[test]
    fn fontconfig_finds_a_sans_serif_font_when_installed() {
        if Command::new("fc-match").arg("--version").output().is_err() {
            return;
        }
        if let Some(path) = best("sans-serif") {
            assert!(path.is_file());
        }
    }
}
