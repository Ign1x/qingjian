//! 全角标点与简繁的切换键走插件那条入口（`LinuxEvent::Key`）：吃掉按键，回包的 `mode` 跟着变。
use crate::support::{compose, key, key_with, router_with};
use qingjian_linux_server::RouterConfig;
use qingjian_platform::KeyCombo;
use qingjian_platform::protocol::KeyModifiers;

#[test]
fn toggle_keys_flip_the_mode_in_the_reply() {
    let mut router = router_with(RouterConfig {
        toggle_punctuation: Some("ctrl+.".parse().unwrap()),
        toggle_traditional: Some(KeyCombo::TRADITIONAL_DEFAULT),
        ..Default::default()
    });
    let ctrl = KeyModifiers {
        ctrl: true,
        ..Default::default()
    };
    let half = key_with(&mut router, 1, b'.' as u32, Some('.'), ctrl, false);
    assert_eq!(half["outcome"], "Consumed");
    assert_eq!(half["mode"]["full_width"], false);
    assert_eq!(
        key(&mut router, 1, b',' as u32, Some(','), false)["outcome"],
        "Passthrough",
        "半角时逗号交给应用"
    );

    compose(&mut router, 1, "ni");
    let shifted = KeyModifiers {
        shift: true,
        ..ctrl
    };
    let traditional = key_with(&mut router, 1, b'F' as u32, Some('F'), shifted, false);
    assert_eq!(traditional["outcome"], "Consumed");
    assert_eq!(traditional["mode"]["traditional"], true);
    assert!(
        !traditional["frame"]["preedit"]
            .as_array()
            .unwrap()
            .is_empty(),
        "组句留着"
    );
}
