//! 按键：触发键进辅码、码段逐键即筛、筛空与退格、Esc、标点先上屏、大写退出辅码、触发键可配。

use qingjian_linux_server::RouterConfig;

use super::{
    BACK, ESCAPE, code_segment, compose, engine, function, key, open, preedit, router, texts,
};

/// 触发键进辅码：候选先不动、拼音行多一个 `;`；敲码逐键收窄，码段按 `AuxCode` 段随帧下发；
/// 筛空时帧照常有拼音行；退格逐级放宽，码删空后停在辅码态，再退格才回拼音。
#[test]
fn the_trigger_filters_candidates_and_marks_the_code_segment() {
    let mut router = router();
    let before = texts(&compose(&mut router, 1, "kaifa"));
    assert!(before.contains(&"开放".to_owned()));

    let entered = key(&mut router, 1, ';');
    assert_eq!(entered["outcome"], "Consumed");
    assert_eq!(entered["commit"], serde_json::Value::Null);
    assert_eq!(preedit(&entered), "kai'fa;");
    assert_eq!(code_segment(&entered), None);
    assert_eq!(texts(&entered), before);

    let filtered = key(&mut router, 1, 'k');
    assert_eq!(preedit(&filtered), "kai'fa;k");
    assert_eq!(code_segment(&filtered).as_deref(), Some("k"));
    assert!(texts(&filtered).contains(&"开发".to_owned()));
    assert!(!texts(&filtered).contains(&"开放".to_owned()));

    let narrowed = key(&mut router, 1, 'f');
    assert_eq!(texts(&narrowed)[0], "开发");
    assert_eq!(
        narrowed["frame"]["candidates"]["items"][0]["aux_code"],
        "kf"
    );

    let empty = key(&mut router, 1, 'q');
    assert!(texts(&empty).is_empty());
    assert_eq!(preedit(&empty), "kai'fa;kfq");

    function(&mut router, 1, BACK);
    function(&mut router, 1, BACK);
    let kept = function(&mut router, 1, BACK);
    assert_eq!(preedit(&kept), "kai'fa;", "码删空后停在辅码态");
    assert_eq!(texts(&kept), before);
    let left = function(&mut router, 1, BACK);
    assert_eq!(preedit(&left), "kai'fa");
}

/// 码段筛空之后，空格、标点、数字都不上屏原始拼音，吞掉停在辅码态等退格；
/// 有候选时标点先上屏高亮候选、再按组句外的规则转全角。
#[test]
fn an_empty_filter_swallows_keys_and_punctuation_commits_first() {
    let mut router = router();
    compose(&mut router, 1, "kaifa;kq");
    for c in [' ', ',', '1'] {
        let swallowed = key(&mut router, 1, c);
        assert_eq!(swallowed["outcome"], "Consumed", "{c}");
        assert_eq!(swallowed["commit"], serde_json::Value::Null, "{c}");
        assert_eq!(preedit(&swallowed), "kai'fa;kq", "{c}");
    }
    let widened = function(&mut router, 1, BACK);
    let highlighted = texts(&widened)[0].clone();
    assert!(highlighted.starts_with("开发"), "{widened}");
    let committed = key(&mut router, 1, ',');
    assert_eq!(committed["commit"], format!("{highlighted}，"));
    assert!(texts(&committed).is_empty());
}

/// Esc 在辅码态只清码段、拼音与候选留着；再敲一次触发键是幂等的。
#[test]
fn escape_clears_only_the_code_and_the_trigger_is_idempotent() {
    let mut router = router();
    compose(&mut router, 1, "kaifa;k");
    let again = key(&mut router, 1, ';');
    assert_eq!(again["commit"], serde_json::Value::Null);
    assert_eq!(preedit(&again), "kai'fa;k");

    let escaped = function(&mut router, 1, ESCAPE);
    assert_eq!(escaped["outcome"], "Consumed");
    assert_eq!(preedit(&escaped), "kai'fa");
    assert_eq!(code_segment(&escaped), None);
    assert!(texts(&escaped).contains(&"开放".to_owned()));
}

/// `shift_letter = "compose"` 时辅码态里敲大写：先清码段回拼音态（同 Esc），字母照常收进拼音。
#[test]
fn an_uppercase_letter_leaves_the_code_before_composing() {
    let config = RouterConfig {
        shift_letter_compose: true,
        ..RouterConfig::default()
    };
    let mut router = open(engine(&config), config, 4);
    compose(&mut router, 1, "kaifa;k");
    let result = key(&mut router, 1, 'G');
    assert_eq!(result["outcome"], "Consumed");
    assert!(!preedit(&result).contains(';'), "{}", preedit(&result));
    assert_eq!(code_segment(&result), None);
}

/// 触发键按 `[general] aux_code_key` 认：换成 `/` 后 `;` 不再进辅码。
#[test]
fn the_trigger_key_follows_the_config() {
    let config = RouterConfig::default();
    let mut slash = engine(&config);
    slash.set_aux_code_key('/', ('[', ']'));
    let mut router = open(slash, config, 4);
    compose(&mut router, 1, "kaifa");
    key(&mut router, 1, ';');
    let semicolon = key(&mut router, 1, 'k');
    assert_eq!(code_segment(&semicolon), None, "{}", preedit(&semicolon));
    function(&mut router, 1, ESCAPE);

    let filtered = compose(&mut router, 1, "kaifa/kh");
    assert_eq!(code_segment(&filtered).as_deref(), Some("kh"));
    assert_eq!(texts(&filtered), ["开"]);
}

/// `[general] aux_code_show` 随帧下发；开着时没进辅码的候选也带着码。
#[test]
fn aux_code_show_rides_the_frame() {
    let plain = compose(&mut router(), 1, "kaifa");
    assert_eq!(plain["frame"]["aux_code_show"], false);

    let config = RouterConfig {
        aux_code_show: true,
        ..RouterConfig::default()
    };
    let mut router = open(engine(&config), config, 4);
    let shown = compose(&mut router, 1, "kaifa");
    assert_eq!(shown["frame"]["aux_code_show"], true);
    let items = shown["frame"]["candidates"]["items"].as_array().unwrap();
    assert!(
        items
            .iter()
            .any(|item| item["text"] == "开发" && item["aux_code"] == "kf")
    );
}
