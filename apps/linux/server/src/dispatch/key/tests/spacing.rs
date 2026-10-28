//! 中文与英文、数字之间自动加空格（`[general] auto_space`）：上文取本会话最近的上屏，还没上屏过时用前端随按键带来的光标前文。
use super::support::{compose, key, router};
use crate::dispatch::Router;
use qingjian_platform::protocol::{KeyModifiers, KeyOutcome};
use serde_json::{Value, json};

fn spaced() -> Router {
    let mut router = router(5);
    router.engine.set_auto_space(true);
    router
}

/// 走插件那条入口按一个键，`before` 是前端截的光标前文（`None` 是应用不报）。
fn linux_key(
    router: &mut Router,
    code: u32,
    character: Option<char>,
    before: Option<&str>,
) -> Value {
    let surrounding = before.map(|before| json!({"before": before, "after": ""}));
    let event =
        json!({"virtual_key": code, "character": character, "modifiers": KeyModifiers::default()});
    router
        .handle_linux(json!({"LinuxEvent": {"session": 1, "event": {"Key": {
            "event": event, "release": false, "surrounding": surrounding
        }}}}))
        .unwrap()["KeyResult"]
        .clone()
}

fn capabilities(router: &mut Router, sensitive: bool) {
    router.handle_linux(
        json!({"LinuxEvent": {"session": 1, "event": {"Capabilities": {
            "sensitive": sensitive, "password": false, "disabled": false
        }}}}),
    );
}

/// 上屏汉字后接着打数字补空格，数字后再打数字不补；退格删掉那个字后不知道上文，不补。
#[test]
fn auto_space_follows_commits_and_backspace() {
    let normal = KeyModifiers::default();
    let mut router = spaced();
    compose(&mut router, "ni", normal);
    assert_eq!(
        key(&mut router, 0x20, Some(' '), normal).1.as_deref(),
        Some("你")
    );
    let (outcome, commit, _) = key(&mut router, 0x33, Some('3'), normal);
    assert_eq!(
        (outcome, commit.as_deref()),
        (KeyOutcome::Consumed, Some(" 3"))
    );
    assert_eq!(
        key(&mut router, 0x34, Some('4'), normal).0,
        KeyOutcome::Passthrough
    );

    let mut router = spaced();
    compose(&mut router, "ni", normal);
    key(&mut router, 0x20, Some(' '), normal);
    assert_eq!(
        key(&mut router, 0x08, None, normal).0,
        KeyOutcome::Passthrough
    );
    assert_eq!(
        key(&mut router, 0x33, Some('3'), normal).0,
        KeyOutcome::Passthrough
    );
}

/// 关着（缺省）不补。
#[test]
fn off_by_default() {
    let normal = KeyModifiers::default();
    let mut router = router(5);
    compose(&mut router, "ni", normal);
    key(&mut router, 0x20, Some(' '), normal);
    assert_eq!(
        key(&mut router, 0x33, Some('3'), normal).0,
        KeyOutcome::Passthrough
    );
}

/// 交给应用的大写字母（中文模式 Shift + 字母）与英文模式不给候选时我们插的字母，同样补空格。
#[test]
fn passed_through_letters_are_spaced_too() {
    let normal = KeyModifiers::default();
    let mut router = spaced();
    compose(&mut router, "ni", normal);
    key(&mut router, 0x20, Some(' '), normal);
    let shift = KeyModifiers {
        shift: true,
        ..normal
    };
    assert_eq!(
        key(&mut router, b'G' as u32, Some('G'), shift).1.as_deref(),
        Some(" G")
    );

    let mut router = spaced();
    router.config.english_candidates = false;
    compose(&mut router, "ni", normal);
    key(&mut router, 0x20, Some(' '), normal);
    let english = KeyModifiers {
        english_mode: true,
        ..normal
    };
    assert_eq!(
        key(&mut router, b'g' as u32, Some('g'), english)
            .1
            .as_deref(),
        Some(" g")
    );
}

/// 这一段还没上屏过：拿前端带来的光标前文判上文；应用不报、或是私密输入框时不知道上文，不补。
#[test]
fn first_commit_uses_the_surrounding_text() {
    for (before, sensitive, expected) in [
        (Some("用"), false, " go"),
        (None, false, "go"),
        (Some(""), false, "go"),
        (Some("用"), true, "go"),
    ] {
        let mut router = spaced();
        capabilities(&mut router, sensitive);
        for c in "go".chars() {
            linux_key(&mut router, c as u32, Some(c), before);
        }
        let result = linux_key(&mut router, 0x0D, None, before);
        assert_eq!(result["commit"], expected, "{before:?} {sensitive}");
    }
}
