//! 键位提示的线上形状：双拼键位原样往返，字根图只带名字，不透明度与大小是百分比。

use qingjian_core::root_chart::RootChart;
use qingjian_core::shuangpin::{KeyHint, Scheme};
use serde_json::json;

use super::{KeyHintContent, KeyHintView, RootChartId};

/// 小鹤双拼敲了 `v`（zh）之后的键盘：`s` 接成 zhong，`b` 接不成。
fn shuangpin() -> KeyHintView {
    KeyHintView {
        content: KeyHintContent::Shuangpin(KeyHint::for_keys(Scheme::Xiaohe.layout(), "nihcv")),
        opacity: 80,
        size: 100,
    }
}

#[test]
fn shuangpin_board_round_trips() {
    let view = shuangpin();
    let value = serde_json::to_value(&view).unwrap();
    let rows = &value["content"]["Shuangpin"]["rows"];
    assert_eq!(rows[0][0]["key"], "q");
    let key = |key: &str| {
        rows.as_array()
            .unwrap()
            .iter()
            .flat_map(|row| row.as_array().unwrap())
            .find(|cap| cap["key"] == key)
            .unwrap()
            .clone()
    };
    assert_eq!(key("s")["state"], json!({"Next": "zhong"}));
    assert_eq!(key("b")["state"], "Unavailable");
    assert_eq!(key("v")["initial"], "zh");
    assert_eq!(value["opacity"], 80);
    assert_eq!(value["size"], 100);
    assert_eq!(serde_json::from_value::<KeyHintView>(value).unwrap(), view);
}

/// 字形不上线：只带字根图的名字，前端从 Core 取同一张。
#[test]
fn root_chart_travels_as_its_name() {
    let id = RootChartId::of(RootChart::wubi86()).expect("Core 的 86 版字根图认得出");
    let view = KeyHintView {
        content: KeyHintContent::Roots(id),
        opacity: 30,
        size: 150,
    };
    let value = serde_json::to_value(&view).unwrap();
    assert_eq!(
        value,
        json!({"content": {"Roots": "wubi86"}, "opacity": 30, "size": 150})
    );
    let KeyHintContent::Roots(id) = serde_json::from_value::<KeyHintView>(value)
        .unwrap()
        .content
    else {
        panic!("应是字根图");
    };
    assert!(std::ptr::eq(id.chart(), RootChart::wubi86()));
}

#[test]
fn percentages_turn_into_alpha_and_zoom() {
    let view = KeyHintView {
        opacity: 30,
        size: 150,
        ..shuangpin()
    };
    assert!((view.alpha() - 0.3).abs() < 1e-6);
    assert!((view.zoom() - 1.5).abs() < 1e-6);
}
