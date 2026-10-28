//! 用户目录 `schemes/` 里的方案：自制双拼方案当拼音侧、自制形码方案当形码侧，回包报它们的名字；选的找不到时退回全拼。

use std::path::Path;

use super::{compose, scheme, started, texts, user_dir};

fn write_scheme(dir: &Path, id: &str, files: &[(&str, &str)]) {
    let scheme = dir.join("schemes").join(id);
    std::fs::create_dir_all(&scheme).unwrap();
    for (name, content) in files {
        std::fs::write(scheme.join(name), content).unwrap();
    }
}

#[test]
fn a_custom_shuangpin_scheme_drives_the_pinyin_side() {
    let dir = user_dir("shuangpin");
    write_scheme(
        &dir,
        "mine",
        &[(
            "scheme.toml",
            "type = \"shuangpin\"\nname = \"我的小鹤\"\nbase = \"xiaohe\"\n",
        )],
    );
    let (mut router, _) = started(&dir, "[general]\nscheme = \"mine\"\n");
    // 小鹤：ni 是 ni，hao 是 hc
    let typed = compose(&mut router, "nihc");
    assert_eq!(texts(&typed)[0], "你好", "{typed}");
    assert_eq!(scheme(&typed), "我的小鹤");
    drop(router);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_custom_shape_scheme_drives_the_code_side() {
    let dir = user_dir("shape");
    write_scheme(
        &dir,
        "huma",
        &[
            ("scheme.toml", "name = \"虎码\"\n"),
            ("table.tsv", "开发\tkf\t100\n你好\tnh\t100\n开\tk\t100\n"),
        ],
    );
    let (mut router, _) = started(&dir, "[general]\nscheme = \"none\"\ncode = \"huma\"\n");
    let first = compose(&mut router, "k");
    assert_eq!(scheme(&first), "虎码");
    assert!(texts(&first).contains(&"开".to_owned()), "{first}");
    // 码表最常见的码长是 2：敲满且唯一就上屏
    assert_eq!(compose(&mut router, "f")["commit"], "开发");
    drop(router);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// 选的方案不在（目录删了）：按全拼打，方案名也按全拼。
#[test]
fn a_missing_scheme_falls_back_to_full_pinyin() {
    let dir = user_dir("missing");
    let (mut router, _) = started(&dir, "[general]\nscheme = \"gone\"\n");
    let typed = compose(&mut router, "nihao");
    assert_eq!(texts(&typed)[0], "你好");
    assert_eq!(scheme(&typed), "");
    drop(router);
    std::fs::remove_dir_all(&dir).unwrap();
}
