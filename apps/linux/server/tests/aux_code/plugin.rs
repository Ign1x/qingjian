//! 发给 Fcitx5 / Fcitx4 插件（握手 v3）的帧：码段当普通拼音段、候选种类都是字符串，
//! 开着「候选上显示码」时码拼在候选文字后面；Rust 前端（v4）原样收，自己画。

use qingjian_core::CustomPhrase;
use qingjian_linux_server::RouterConfig;
use serde_json::Value;

use super::{compose, engine, key, open, preedit, texts};

fn items(result: &Value) -> &Vec<Value> {
    result["frame"]["candidates"]["items"].as_array().unwrap()
}

fn segment_kinds(result: &Value) -> Vec<String> {
    result["frame"]["preedit"]
        .as_array()
        .unwrap()
        .iter()
        .map(|segment| segment["kind"].as_str().unwrap().to_owned())
        .collect()
}

/// Fcitx5 插件按字符串取候选的 `kind`，自定义短语的 `{"Custom": n}` 会让它抛异常、断开整条连接；
/// 码段 `AuxCode` 是 v3 之后才有的。v3 的会话两样都收不到，v4 的原样收。
#[test]
fn plugins_get_frames_in_the_v3_vocabulary() {
    for version in [3, 4] {
        let config = RouterConfig::default();
        let mut engine = engine(&config);
        engine
            .set_custom_phrases(vec![CustomPhrase::new("kaifa", "开发者大会")])
            .unwrap();
        let mut router = open(engine, config, version);
        let typed = compose(&mut router, 1, "kaifa");
        assert!(texts(&typed).contains(&"开发者大会".to_owned()), "{typed}");
        let objects = items(&typed)
            .iter()
            .filter(|item| !item["kind"].is_string())
            .count();
        assert_eq!(objects, usize::from(version == 4), "v{version}：{typed}");

        let coded = compose(&mut router, 1, ";k");
        assert_eq!(preedit(&coded), "kai'fa;k");
        assert_eq!(
            segment_kinds(&coded).contains(&"AuxCode".to_owned()),
            version == 4,
            "v{version}：{coded}"
        );
    }
}

/// 开着「候选上显示码」时插件的面板只有文字与注记两栏，码拼在候选文字后面；选词按下标回 Server，上屏的仍是原词。
#[test]
fn plugins_see_codes_after_the_candidate_text() {
    for version in [3, 4] {
        let config = RouterConfig {
            aux_code_show: true,
            ..RouterConfig::default()
        };
        let mut router = open(engine(&config), config, version);
        let typed = compose(&mut router, 1, "kaifa");
        let shown = if version == 3 { "开发[kf]" } else { "开发" };
        let index = texts(&typed)
            .iter()
            .position(|text| text == shown)
            .unwrap_or_else(|| panic!("v{version}：{typed}"));
        let digit = char::from_digit(index as u32 + 1, 10).unwrap();
        assert_eq!(key(&mut router, 1, digit)["commit"], "开发");
    }
}
