//! 键位提示：只给握手报了 `features.key_hint` 的会话附在回包上；「卡住时显示」停顿到点由轮询带出来，展示身份跟着换新。

use std::time::Duration;

use qingjian_core::Engine;
use qingjian_core::shuangpin::Scheme;
use qingjian_dictionary::Dictionary;
use qingjian_platform::protocol::{
    ClientMessage, KeyEvent, KeyModifiers, PROTOCOL_VERSION, SessionId,
};
use qingjian_platform::{DEFAULT_KEY_HINT_OPACITY, KeyHintMode};
use serde_json::{Value, json};

use crate::dispatch::{Router, RouterConfig};

/// 小鹤双拼，会话 1、2 都握好手（`features` 照给的报）、报过普通输入框的能力。
fn router(mode: KeyHintMode, features: Value) -> Router {
    let mut engine = Engine::new(Dictionary::parse("你\tni\t100\n").unwrap());
    engine.set_shuangpin(Some(Scheme::Xiaohe));
    let mut router = Router::new(
        engine,
        RouterConfig {
            key_hint: mode,
            ..RouterConfig::default()
        },
    );
    for id in [1, 2] {
        router.handle(ClientMessage::OpenSession {
            session: SessionId(id),
            app: None,
            protocol: PROTOCOL_VERSION,
        });
        router.handle_linux(json!({"DisplayReporting": {
            "session": id,
            "identity": {"generation": 1, "context": format!("key-hint-{id}"), "revision": 0},
            "features": features,
        }}));
        event(
            &mut router,
            id,
            json!({"Capabilities": {"sensitive": false, "password": false, "disabled": false}}),
        );
    }
    router
}

fn event(router: &mut Router, id: u64, event: Value) -> Value {
    router
        .handle_linux(json!({"LinuxEvent": {"session": id, "event": event}}))
        .unwrap()["KeyResult"]
        .clone()
}

fn press(router: &mut Router, id: u64, code: u32, character: Option<char>) -> Value {
    let key = KeyEvent::new(code, character, KeyModifiers::default());
    event(
        router,
        id,
        json!({"Key": {"event": key, "release": false, "surrounding": null}}),
    )
}

fn type_keys(router: &mut Router, id: u64, keys: &str) -> Value {
    keys.chars()
        .map(|c| press(router, id, c as u32, Some(c)))
        .last()
        .unwrap()
}

/// Esc 清掉组句。
fn escape(router: &mut Router, id: u64) -> Value {
    press(router, id, 0x1B, None)
}

fn poll(router: &mut Router, id: u64) -> Value {
    router
        .handle_linux(json!({"Poll": {"session": id}}))
        .unwrap()["Update"]
        .clone()
}

fn revision(body: &Value) -> u64 {
    body["identity"]["revision"].as_u64().unwrap()
}

/// Fcitx5 / Fcitx4 插件与 IBus 前端不报 `features`：开着「一直显示」也不附，回包形状不变。
#[test]
fn sessions_that_cannot_draw_it_get_nothing() {
    for features in [json!(null), json!({"grid": true})] {
        let mut router = router(KeyHintMode::Always, features);
        let typed = type_keys(&mut router, 1, "k");
        assert!(!typed["frame"]["preedit"].as_array().unwrap().is_empty());
        assert!(typed.get("key_hint").is_none());
        assert!(poll(&mut router, 1).get("key_hint").is_none());
    }
}

#[test]
fn always_attaches_the_shuangpin_keys_while_composing() {
    let mut router = router(KeyHintMode::Always, json!({"key_hint": true}));
    let typed = type_keys(&mut router, 1, "k");
    let hint = &typed["key_hint"];
    assert_eq!(hint["content"]["Shuangpin"]["pending"], "k");
    assert_eq!(hint["opacity"], DEFAULT_KEY_HINT_OPACITY);
    assert_eq!(hint["size"], 100);
    let cleared = escape(&mut router, 1);
    assert!(cleared["frame"]["preedit"].as_array().unwrap().is_empty());
    assert!(cleared.get("key_hint").is_none(), "组句结束收起键盘");
}

#[test]
fn off_never_attaches_keys() {
    let mut router = router(KeyHintMode::Off, json!({"key_hint": true}));
    assert!(type_keys(&mut router, 1, "k").get("key_hint").is_none());
}

/// 「卡住时显示」：刚敲完不弹；停顿到点后的那次轮询带上键盘、展示身份换新（前端才会重画），之后照常沿用；
/// 这一段组句里一直留着，下一段重新计时。
#[test]
fn auto_pops_up_on_the_poll_after_a_pause() {
    let mut router = router(KeyHintMode::Auto, json!({"key_hint": true}));
    let typed = type_keys(&mut router, 1, "k");
    assert!(typed.get("key_hint").is_none(), "刚敲完不弹");
    let early = poll(&mut router, 1);
    assert!(early.get("key_hint").is_none());
    assert_eq!(
        revision(&early),
        revision(&typed),
        "没到点：帧没变，身份沿用"
    );

    std::thread::sleep(Duration::from_millis(750));
    let popped = poll(&mut router, 1);
    assert!(popped.get("key_hint").is_some(), "停顿到点弹出");
    assert!(revision(&popped) > revision(&typed), "弹出要换展示身份");
    let again = poll(&mut router, 1);
    assert!(again.get("key_hint").is_some());
    assert_eq!(revision(&again), revision(&popped));

    assert!(
        type_keys(&mut router, 1, "a").get("key_hint").is_some(),
        "这一段组句里留着"
    );
    escape(&mut router, 1);
    assert!(
        type_keys(&mut router, 1, "k").get("key_hint").is_none(),
        "下一段重新计时"
    );
}

/// 敲出配不成音节的键（小鹤 `vb`）：多半是记不清键位，当场弹出。换到别的会话重新计时，不跟着弹。
#[test]
fn auto_pops_up_at_once_on_a_stuck_key() {
    let mut router = router(KeyHintMode::Auto, json!({"key_hint": true}));
    assert!(type_keys(&mut router, 1, "niv").get("key_hint").is_none());
    let stuck = type_keys(&mut router, 1, "b");
    assert_eq!(stuck["key_hint"]["content"]["Shuangpin"]["stuck"], true);
    assert!(type_keys(&mut router, 2, "k").get("key_hint").is_none());
}
