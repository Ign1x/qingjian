//! 回包上的 `mode` 与托盘切换（`Indicator`）：中 / 英缺省共用、可按应用记，全角标点按当前模式那一份、简繁全局并当场换字形。
use crate::support::{compose, event, key, router, router_on, router_with};
use qingjian_core::{Engine, ShuangpinScheme};
use qingjian_dictionary::Dictionary;
use qingjian_linux_server::{Router, RouterConfig};
use qingjian_platform::{Config, Scheme, SwitchKey, SwitchKeys};
use serde_json::{Value, json};

const SHIFT: u32 = 0x10;

fn indicator(router: &mut Router, id: u64, command: &str) -> Value {
    event(router, id, json!({ "Indicator": command }))
}

fn mode(result: &Value) -> (bool, bool, bool, String) {
    let mode = &result["mode"];
    (
        mode["english"].as_bool().expect("english"),
        mode["traditional"].as_bool().expect("traditional"),
        mode["full_width"].as_bool().expect("full_width"),
        mode["scheme"].as_str().expect("scheme").to_owned(),
    )
}

#[test]
fn every_reply_carries_its_sessions_mode() {
    let mut router = router();
    let chinese = (false, false, true, String::new());
    assert_eq!(mode(&compose(&mut router, 1, "ni")), chinese);
    let polled = router
        .handle_linux(json!({"Poll": {"session": 1}}))
        .unwrap();
    assert_eq!(mode(&polled["Update"]), chinese);

    // 缺省所有会话共用一份：单击 Shift 切会话 2，会话 1 的回包也是英文
    key(&mut router, 2, SHIFT, None, false);
    let switched = key(&mut router, 2, SHIFT, None, true);
    assert_eq!(switched["english"], true);
    assert_eq!(mode(&switched), (true, false, false, String::new()));
    assert!(mode(&key(&mut router, 1, 0x1b, None, false)).0);

    // 按应用记：两个会话都没报应用名，各记各的，切会话 2 不动会话 1
    let mut router = router_with(RouterConfig {
        remember_mode_per_app: true,
        switch_mode: SwitchKeys::NONE.with(SwitchKey::Shift, true),
        ..Default::default()
    });
    key(&mut router, 2, SHIFT, None, false);
    assert_eq!(key(&mut router, 2, SHIFT, None, true)["english"], true);
    assert!(!mode(&key(&mut router, 1, 0x1b, None, false)).0);
}

#[test]
fn indicator_toggles_english_like_the_switch_key() {
    let mut router = router();
    compose(&mut router, 1, "ni");
    let english = indicator(&mut router, 1, "ToggleEnglish");
    assert_eq!(english["outcome"], "Consumed");
    assert_eq!(english["english"], true, "前端拿它弹「英」");
    assert_eq!(english["commit"], "ni", "组句中的拼音原样上屏");
    assert!(english["frame"]["preedit"].as_array().unwrap().is_empty());
    assert!(mode(&english).0);
    assert_eq!(
        compose(&mut router, 1, "n")["commit"],
        "n",
        "英文模式直接上屏"
    );

    let back = indicator(&mut router, 1, "ToggleEnglish");
    assert_eq!(back["english"], false);
    assert!(!mode(&back).0);

    // 关掉内置英文模式：托盘也不切
    let mut router = router_with(RouterConfig {
        english_mode: false,
        ..Default::default()
    });
    let refused = indicator(&mut router, 1, "ToggleEnglish");
    assert_eq!(refused["outcome"], "Passthrough");
    assert_eq!(refused["english"], json!(null));
    assert!(!mode(&refused).0);
}

#[test]
fn indicator_toggles_the_current_modes_punctuation() {
    let mut router = router();
    let half = indicator(&mut router, 1, "TogglePunctuation");
    assert_eq!(half["outcome"], "Consumed");
    assert!(!mode(&half).2);
    assert_eq!(
        key(&mut router, 1, 0xBC, Some(','), false)["outcome"],
        "Passthrough",
        "半角时逗号交给应用"
    );

    // 英文模式那一份单独记：切过去是半角，翻成全角不动中文那一份
    assert!(!mode(&indicator(&mut router, 1, "ToggleEnglish")).2);
    assert!(mode(&indicator(&mut router, 1, "TogglePunctuation")).2);
    assert!(!mode(&indicator(&mut router, 1, "ToggleEnglish")).2);
}

#[test]
fn indicator_toggles_traditional_and_redraws_the_candidates() {
    let engine = Engine::new(Dictionary::parse("汉\than\t100\n").unwrap());
    let mut router = router_on(engine, RouterConfig::default());
    assert_eq!(
        compose(&mut router, 1, "han")["frame"]["candidates"]["items"][0]["text"],
        "汉"
    );
    let traditional = indicator(&mut router, 1, "ToggleTraditional");
    assert_eq!(traditional["outcome"], "Consumed");
    assert!(mode(&traditional).1);
    assert_eq!(
        traditional["frame"]["candidates"]["items"][0]["text"], "漢",
        "组句中的候选当场换字形"
    );
    let simplified = indicator(&mut router, 1, "ToggleTraditional");
    assert!(!mode(&simplified).1);
    assert_eq!(simplified["frame"]["candidates"]["items"][0]["text"], "汉");
}

#[test]
fn toggles_are_written_back_to_the_config() {
    let directory =
        std::env::temp_dir().join(format!("qingjian-linux-mode-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("config.toml");
    std::fs::write(
        &path,
        "# 注释要留着\n[general]\nlast_traditional = \"hongkong\"\n",
    )
    .unwrap();
    let config = Config::load(&path).unwrap();
    let mut router = router_with(RouterConfig::from(&config));
    router.watch_config(&config, path.clone(), directory.clone(), directory.clone());

    indicator(&mut router, 1, "TogglePunctuation");
    indicator(&mut router, 1, "ToggleTraditional");
    let written = Config::load(&path).unwrap().general;
    assert!(!written.full_width_punctuation);
    assert_eq!(
        written.traditional_variant, "hongkong",
        "切回上次用的那一档繁体"
    );
    indicator(&mut router, 1, "ToggleTraditional");
    let written = Config::load(&path).unwrap().general;
    assert_eq!(written.traditional_variant, "off");
    assert_eq!(written.last_traditional, "hongkong");
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .contains("# 注释要留着")
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn scheme_label_follows_the_configured_pinyin_side() {
    for (scheme, label) in [
        (Scheme::Pinyin, ""),
        (Scheme::Shuangpin(ShuangpinScheme::Xiaohe), "小鹤双拼"),
        (Scheme::Zhuyin, "大千注音"),
        // 没装上用户双拼方案（这里没配方案目录），引擎按全拼跑，方案名也按全拼
        (Scheme::Custom, ""),
    ] {
        let mut router = router_with(RouterConfig {
            scheme,
            ..Default::default()
        });
        assert_eq!(mode(&key(&mut router, 1, 0x1b, None, false)).3, label);
    }
}
