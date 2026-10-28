//! 带调拼音与键位提示的配置热加载。

use super::{poll, temp_dir, watched, write_config};
use crate::dispatch::{Router, RouterConfig};
use qingjian_core::Engine;
use qingjian_dictionary::Dictionary;
use qingjian_platform::Config;
use qingjian_platform::protocol::{
    ClientMessage, KeyEvent, KeyModifiers, PROTOCOL_VERSION, SessionId,
};

/// 拼音行显示声调改了配置就换：窗口里那一串带不带调跟着热加载走。
#[test]
fn show_tones_follows_the_config() {
    let dir = temp_dir("tones");
    let config_path = dir.join("config.toml");
    write_config(&config_path, "", 100);
    let config = Config::load(&config_path).unwrap();
    let mut router = Router::new(
        Engine::new(Dictionary::parse("你好\tni hao\t100\n").unwrap()),
        RouterConfig::from(&config),
    );
    router.watch_config(&config, config_path.clone(), dir.clone(), dir.clone());
    router.handle(ClientMessage::OpenSession {
        session: SessionId(1),
        app: None,
        protocol: PROTOCOL_VERSION,
    });
    for c in "nihao".chars() {
        router.handle(ClientMessage::Key {
            session: SessionId(1),
            event: KeyEvent::new(c as u32, Some(c), KeyModifiers::default()),
        });
    }
    assert_eq!(router.current_frame().window_preedit, None);

    write_config(&config_path, "[general]\nshow_tones = true\n", 200);
    poll(&mut router);
    assert!(router.current_frame().window_preedit.is_some());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn key_hint_and_look_follow_reloaded_config() {
    use serde_json::json;

    let dir = temp_dir("key-hint");
    let mut router = watched(&dir, "[general]\nscheme = \"xiaohe\"\nkey_hint = \"off\"\n");
    router
        .engine
        .set_shuangpin(Some(qingjian_core::shuangpin::Scheme::Xiaohe));
    router.handle(ClientMessage::OpenSession {
        session: SessionId(1),
        app: None,
        protocol: PROTOCOL_VERSION,
    });
    router.handle_linux(json!({"DisplayReporting": {"session": 1,
        "identity": {"generation": 1, "context": "reload", "revision": 0},
        "features": {"key_hint": true}}}));
    let key = router
        .handle_linux(json!({"Key": {"session": 1,
        "event": KeyEvent::new('k' as u32, Some('k'), KeyModifiers::default())}}))
        .unwrap();
    assert!(key["KeyResult"].get("key_hint").is_none());
    write_config(
        &dir.join("config.toml"),
        "[general]\nscheme = \"xiaohe\"\nkey_hint = \"always\"\nkey_hint_opacity = 90\nkey_hint_size = 140\n",
        200,
    );
    poll(&mut router);
    let shown = router
        .handle_linux(json!({"Poll": {"session": 1}}))
        .unwrap();
    assert_eq!(shown["Update"]["key_hint"]["opacity"], 90);
    assert_eq!(shown["Update"]["key_hint"]["size"], 140);
    assert!(
        shown["Update"]["identity"]["revision"].as_u64().unwrap()
            > key["KeyResult"]["identity"]["revision"].as_u64().unwrap()
    );
    write_config(
        &dir.join("config.toml"),
        "[general]\nscheme = \"xiaohe\"\nkey_hint = \"off\"\n",
        300,
    );
    poll(&mut router);
    let hidden = router
        .handle_linux(json!({"Poll": {"session": 1}}))
        .unwrap();
    assert!(hidden["Update"].get("key_hint").is_none());
    assert!(
        hidden["Update"]["identity"]["revision"].as_u64().unwrap()
            > shown["Update"]["identity"]["revision"].as_u64().unwrap()
    );
    std::fs::remove_dir_all(dir).unwrap();
}
