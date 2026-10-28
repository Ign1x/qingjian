//! 辅码与横排矩阵、带调拼音行和键位提示共用同一份会话输入状态。

use qingjian_core::shuangpin::Scheme as ShuangpinScheme;
use qingjian_linux_server::{Router, RouterConfig};
use qingjian_platform::{KeyHintMode, LayoutMode, Scheme};
use serde_json::{Value, json};

use super::{ESCAPE, code_segment, compose, engine, function, open, preedit, texts};

fn display_router() -> Router {
    let scheme = ShuangpinScheme::Xiaohe;
    let config = RouterConfig {
        page_size: 1,
        layout: LayoutMode::Horizontal,
        horizontal_grid: true,
        key_hint: KeyHintMode::Always,
        scheme: Scheme::Shuangpin(scheme),
        ..RouterConfig::default()
    };
    let mut engine = engine(&config);
    engine.set_shuangpin(Some(scheme));
    engine.set_show_tones(true);
    let mut router = open(engine, config, 4);
    for id in [1, 2] {
        router.handle_linux(json!({"DisplayReporting": {
            "session": id, "version": 4,
            "identity": {"generation": 1, "context": format!("aux-grid-{id}"), "revision": 0},
            "features": {"grid": true, "key_hint": true}
        }}));
    }
    router
}

fn poll(router: &mut Router, id: u64) -> Value {
    router
        .handle_linux(json!({"Poll": {"session": id}}))
        .unwrap()["Update"]
        .clone()
}

#[test]
fn escape_collapses_the_grid_before_clearing_the_aux_code() {
    let mut router = display_router();
    let pinyin = compose(&mut router, 1, "kdfa");
    assert!(pinyin["key_hint"]["content"]["Shuangpin"].is_object());
    compose(&mut router, 1, ";k");
    let expanded = function(&mut router, 1, 0x28);
    assert_eq!(expanded["frame"]["columns"], 1);
    assert_eq!(code_segment(&expanded).as_deref(), Some("k"));
    let selected =
        texts(&expanded)[expanded["frame"]["highlight"].as_u64().unwrap() as usize].clone();
    let window = expanded["frame"]["window_preedit"][0].as_array().unwrap();
    let toned: String = window
        .iter()
        .map(|segment| segment["text"].as_str().unwrap())
        .collect();
    assert!(toned.contains("kāi"), "{expanded}");
    assert!(window.iter().any(|segment| segment["kind"] == "AuxCode"));
    assert!(expanded.get("key_hint").is_none(), "辅码态不画双拼键盘");

    let collapsed = function(&mut router, 1, ESCAPE);
    assert_eq!(collapsed["frame"]["columns"], 0);
    assert_eq!(code_segment(&collapsed).as_deref(), Some("k"));
    assert_eq!(texts(&collapsed), [selected]);
    assert_eq!(preedit(&collapsed), preedit(&expanded));

    let cleared = function(&mut router, 1, ESCAPE);
    assert_eq!(preedit(&cleared), "kai'fa");
    assert_eq!(code_segment(&cleared), None);
    assert!(!texts(&cleared).is_empty());
    assert!(cleared["key_hint"]["content"]["Shuangpin"].is_object());
    assert!(texts(&function(&mut router, 1, ESCAPE)).is_empty());
}

#[test]
fn aux_code_and_grid_resume_together_without_filtering_the_other_session() {
    let mut router = display_router();
    compose(&mut router, 1, "kdfa;k");
    let expanded = function(&mut router, 1, 0x28);

    let other = compose(&mut router, 2, "kdfa");
    assert_eq!(preedit(&other), "kai'fa");
    assert_eq!(code_segment(&other), None);
    assert_eq!(other["frame"]["columns"], 0);
    assert!(other["key_hint"]["content"]["Shuangpin"].is_object());

    let resumed = poll(&mut router, 1);
    assert_eq!(resumed["frame"], expanded["frame"]);
    assert!(resumed.get("key_hint").is_none());
    assert_eq!(code_segment(&resumed).as_deref(), Some("k"));
}
