//! 热加载：改了 `[general] code` 下一拍换上形码；导入五笔词库（用户 `code_dicts/`）不等配置改动就并进码表。

use std::path::Path;
use std::time::{Duration, SystemTime};

use qingjian_linux_server::Router;
use qingjian_platform::Config;
use qingjian_platform::protocol::{ClientMessage, PROTOCOL_VERSION, SessionId};

use super::{compose, key, scheme, started, texts, user_dir};
use serde_json::json;

/// 写配置并钉死 mtime：同一秒内连写两次时文件系统的 mtime 可能不变。
fn rewrite(path: &Path, source: &str, seconds: u64) {
    std::fs::write(path, source).unwrap();
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds))
        .unwrap();
}

/// 开热加载：配置在 `dir/config.toml`，用户数据目录也是 `dir`。
fn watched(router: &mut Router, dir: &Path, config: &Config) {
    router.watch_config(
        config,
        dir.join("config.toml"),
        dir.to_owned(),
        dir.to_owned(),
    );
}

/// 热加载一秒只看一次，先等过这一秒再走一拍。
fn next_tick(router: &mut Router) {
    std::thread::sleep(Duration::from_millis(1100));
    router.tick();
}

#[test]
fn turning_on_wubi_applies_on_the_next_tick() {
    let dir = user_dir("reload-code");
    let (mut router, config) = started(&dir, "[general]\n");
    watched(&mut router, &dir, &config);
    let before = compose(&mut router, "ggll");
    assert!(!texts(&before).contains(&"一".to_owned()));
    assert_eq!(scheme(&before), "");
    key(&mut router, ' ');

    rewrite(
        &dir.join("config.toml"),
        "[general]\nscheme = \"none\"\ncode = \"wubi86\"\n",
        200,
    );
    next_tick(&mut router);
    let after = compose(&mut router, "ggll");
    assert_eq!(texts(&after)[0], "一");
    assert_eq!(scheme(&after), "五笔（86）");
    drop(router);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_imported_wubi_dictionary_applies_without_a_config_change() {
    let dir = user_dir("reload-words");
    let (mut router, config) = started(&dir, "[general]\nscheme = \"none\"\ncode = \"wubi86\"\n");
    watched(&mut router, &dir, &config);
    assert!(texts(&compose(&mut router, "zzy")).is_empty());
    key(&mut router, ' ');

    let words = dir.join("code_dicts");
    std::fs::create_dir_all(&words).unwrap();
    std::fs::write(words.join("mine.tsv"), "青简\tzzyy\t1000\n").unwrap();
    next_tick(&mut router);
    assert_eq!(texts(&compose(&mut router, "zzy")), ["青简"]);
    drop(router);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn editing_a_shape_table_refreshes_the_current_candidates() {
    let dir = user_dir("reload-table");
    let scheme_dir = dir.join("schemes/mine");
    std::fs::create_dir_all(&scheme_dir).unwrap();
    std::fs::write(scheme_dir.join("scheme.toml"), "name = \"我的形码\"\n").unwrap();
    rewrite(&scheme_dir.join("table.tsv"), "青简\tqq\t100\n", 100);
    let (mut router, config) = started(&dir, "[general]\nscheme = \"none\"\ncode = \"mine\"\n");
    watched(&mut router, &dir, &config);
    assert_eq!(texts(&compose(&mut router, "q")), ["青简"]);

    rewrite(&scheme_dir.join("table.tsv"), "清减\tqq\t100\n", 200);
    next_tick(&mut router);
    let updated = router
        .handle_linux(json!({"Poll": {"session": 1}}))
        .unwrap();
    assert_eq!(texts(&updated["Update"]), ["清减"], "{updated}");
    assert_eq!(key(&mut router, ' ')["commit"], "清减");
    drop(router);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn editing_a_custom_shuangpin_scheme_applies_without_a_config_change() {
    let dir = user_dir("reload-shuangpin");
    let scheme_dir = dir.join("schemes/mine");
    std::fs::create_dir_all(&scheme_dir).unwrap();
    rewrite(
        &scheme_dir.join("scheme.toml"),
        "type = \"shuangpin\"\nname = \"小鹤底\"\nbase = \"xiaohe\"\n",
        100,
    );
    let (mut router, config) = started(&dir, "[general]\nscheme = \"mine\"\n");
    watched(&mut router, &dir, &config);
    assert_eq!(scheme(&compose(&mut router, "nihc")), "小鹤底");
    key(&mut router, ' ');

    rewrite(
        &scheme_dir.join("scheme.toml"),
        "type = \"shuangpin\"\nname = \"自然码底\"\nbase = \"ziranma\"\n",
        200,
    );
    next_tick(&mut router);
    let after = compose(&mut router, "nihk");
    assert_eq!(texts(&after)[0], "你好", "{after}");
    assert_eq!(scheme(&after), "自然码底");
    drop(router);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn editing_a_shape_table_refreshes_a_suspended_composition() {
    let dir = user_dir("reload-suspended");
    let scheme_dir = dir.join("schemes/mine");
    std::fs::create_dir_all(&scheme_dir).unwrap();
    std::fs::write(scheme_dir.join("scheme.toml"), "name = \"我的形码\"\n").unwrap();
    rewrite(&scheme_dir.join("table.tsv"), "青简\tqq\t100\n", 100);
    let (mut router, config) = started(&dir, "[general]\nscheme = \"none\"\ncode = \"mine\"\n");
    watched(&mut router, &dir, &config);
    assert_eq!(texts(&compose(&mut router, "q")), ["青简"]);

    router.handle(ClientMessage::OpenSession {
        session: SessionId(2),
        app: None,
        protocol: PROTOCOL_VERSION,
    });
    router.handle_linux(json!({"LinuxEvent": {"session": 2, "event": {
        "Focus": {"focused": true}
    }}}));
    rewrite(&scheme_dir.join("table.tsv"), "清减\tqq\t100\n", 200);
    next_tick(&mut router);

    let resumed = router
        .handle_linux(json!({"Poll": {"session": 1}}))
        .unwrap();
    assert_eq!(texts(&resumed["Update"]), ["清减"]);
    assert_eq!(key(&mut router, ' ')["commit"], "清减");
    drop(router);
    std::fs::remove_dir_all(dir).unwrap();
}
