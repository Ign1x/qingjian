//! 回包上的 `mode`：中 / 英按会话，全角标点取当前模式那一份，方案名按配置的拼音侧。
use crate::support::{compose, key, router, router_with};
use qingjian_core::ShuangpinScheme;
use qingjian_linux_server::RouterConfig;
use qingjian_platform::Scheme;
use serde_json::{Value, json};

const SHIFT: u32 = 0x10;

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

    // 单击 Shift 只切会话 2；会话 1 的回包仍是中文
    key(&mut router, 2, SHIFT, None, false);
    let switched = key(&mut router, 2, SHIFT, None, true);
    assert_eq!(switched["english"], true);
    assert_eq!(mode(&switched), (true, false, false, String::new()));
    assert!(!mode(&key(&mut router, 1, 0x1b, None, false)).0);
}

#[test]
fn scheme_label_follows_the_configured_pinyin_side() {
    for (scheme, label) in [
        (Scheme::Pinyin, ""),
        (Scheme::Shuangpin(ShuangpinScheme::Xiaohe), "小鹤双拼"),
        (Scheme::Zhuyin, "大千注音"),
        // Linux 还不装用户双拼，引擎按全拼跑，方案名也按全拼
        (Scheme::Custom, ""),
    ] {
        let mut router = router_with(RouterConfig {
            scheme,
            ..Default::default()
        });
        assert_eq!(mode(&key(&mut router, 1, 0x1b, None, false)).3, label);
    }
}
