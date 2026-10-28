//! 热加载：启动时按 `[aux_code]` 从用户目录 `codes/` 装码表，之后导入新码表、改选另一套都不用重启。

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use qingjian_dictionary::AuxCodeTable;
use qingjian_linux_server::{AssemblySpec, Router, RouterConfig, assembly};
use qingjian_platform::Config;

use super::{DICT, ESCAPE, code_segment, compose, function, open, texts};
use serde_json::json;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("qingjian-aux-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("codes")).unwrap();
    dir
}

/// 写配置并钉死 mtime：同一秒内连写两次时文件系统的 mtime 可能不变。
fn write_config(path: &Path, source: &str, seconds: u64) -> Config {
    std::fs::write(path, source).unwrap();
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds))
        .unwrap();
    Config::load(path).unwrap()
}

fn write_table(path: &Path, code: &str) {
    AuxCodeTable::from_pairs([("开发".to_owned(), code.to_owned())])
        .unwrap()
        .write_qj(path, &Default::default())
        .unwrap();
}

/// 照 `main.rs` 装配：用户目录就是 `dir`，随包根也指过去（那里没有随包码表）。
fn started(dir: &Path, config: &Config) -> Router {
    let dict = dir.join("dict.tsv");
    std::fs::write(&dict, DICT).unwrap();
    let mut engine = assembly::assemble(&AssemblySpec {
        user_dir: Some(dir.to_owned()),
        aux_code: config.aux_code.clone(),
        ..AssemblySpec::new(dict)
    })
    .unwrap();
    engine.set_aux_enabled(config.aux_code.enabled);
    let mut router = open(engine, RouterConfig::from(config), 4);
    router.watch_config(
        config,
        dir.join("config.toml"),
        dir.to_owned(),
        dir.to_owned(),
    );
    router
}

/// 热加载一秒只看一次。
fn next_tick(router: &mut Router) {
    std::thread::sleep(Duration::from_millis(1100));
    router.tick();
}

/// 设置页刚导入一张码表：`codes/` 里出现新文件就装上，不等配置改动。
#[test]
fn a_new_table_in_the_user_dir_applies_on_the_next_tick() {
    let dir = temp_dir("import");
    let config = write_config(
        &dir.join("config.toml"),
        "[aux_code]\nenabled = true\n",
        100,
    );
    let mut router = started(&dir, &config);
    let before = compose(&mut router, 1, "kaifa;k");
    assert_eq!(code_segment(&before), None, "一张码表都没有时 ; 照旧");
    function(&mut router, 1, ESCAPE);

    write_table(&dir.join("codes/mine.qj"), "kf");
    next_tick(&mut router);
    let after = compose(&mut router, 1, "kaifa;k");
    assert_eq!(code_segment(&after).as_deref(), Some("k"));
    assert_eq!(texts(&after), ["开发"]);
    // Router 收工时落盘学习数据，先收工再删目录
    drop(router);
    std::fs::remove_dir_all(&dir).unwrap();
}

/// 启动时按 `[aux_code] scheme` 挑一套；改选另一套后下一拍换过去。
#[test]
fn switching_the_scheme_reloads_the_table() {
    let dir = temp_dir("switch");
    write_table(&dir.join("codes/first.qj"), "kf");
    write_table(&dir.join("codes/second.qj"), "zz");
    let path = dir.join("config.toml");
    let config = write_config(
        &path,
        "[aux_code]\nenabled = true\nscheme = \"first\"\n",
        100,
    );
    let mut router = started(&dir, &config);
    assert_eq!(texts(&compose(&mut router, 1, "kaifa;k")), ["开发"]);
    function(&mut router, 1, ESCAPE);
    function(&mut router, 1, ESCAPE);

    write_config(
        &path,
        "[aux_code]\nenabled = true\nscheme = \"second\"\n",
        200,
    );
    next_tick(&mut router);
    assert_eq!(texts(&compose(&mut router, 1, "kaifa;z")), ["开发"]);
    function(&mut router, 1, ESCAPE);
    function(&mut router, 1, ESCAPE);
    assert!(texts(&compose(&mut router, 1, "kaifa;k")).is_empty());
    drop(router);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn switching_tables_refreshes_aux_input_in_suspended_sessions() {
    let dir = temp_dir("suspended");
    write_table(&dir.join("codes/first.qj"), "kf");
    write_table(&dir.join("codes/second.qj"), "zz");
    let path = dir.join("config.toml");
    let config = write_config(
        &path,
        "[aux_code]\nenabled = true\nscheme = \"first\"\n",
        100,
    );
    let mut router = started(&dir, &config);
    assert_eq!(texts(&compose(&mut router, 1, "kaifa;k")), ["开发"]);
    compose(&mut router, 2, "kaifa");
    write_config(
        &path,
        "[aux_code]\nenabled = true\nscheme = \"second\"\n",
        200,
    );
    next_tick(&mut router);

    let resumed = router
        .handle_linux(json!({"Poll": {"session": 1}}))
        .unwrap();
    assert_eq!(code_segment(&resumed["Update"]), None);
    assert!(texts(&resumed["Update"]).contains(&"开发".to_owned()));
    assert_eq!(texts(&compose(&mut router, 1, ";z")), ["开发"]);
    drop(router);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn showing_codes_refreshes_current_and_suspended_candidates() {
    for suspended in [false, true] {
        let dir = temp_dir(if suspended {
            "show-suspended"
        } else {
            "show-active"
        });
        write_table(&dir.join("codes/first.qj"), "kf");
        let path = dir.join("config.toml");
        let config = write_config(&path, "[aux_code]\nenabled = true\n", 100);
        let mut router = started(&dir, &config);
        compose(&mut router, 1, "kaifa");
        if suspended {
            compose(&mut router, 2, "kaifa");
        }
        write_config(
            &path,
            "[general]\naux_code_show = true\n[aux_code]\nenabled = true\n",
            200,
        );
        next_tick(&mut router);
        let updated = router
            .handle_linux(json!({"Poll": {"session": 1}}))
            .unwrap();
        assert_eq!(updated["Update"]["frame"]["aux_code_show"], true);
        let candidates = updated["Update"]["frame"]["candidates"]["items"]
            .as_array()
            .unwrap();
        assert!(
            candidates
                .iter()
                .any(|item| item["text"] == "开发" && item["aux_code"] == "kf")
        );
        drop(router);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
