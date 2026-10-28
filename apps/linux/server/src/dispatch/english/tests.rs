//! 中英模式的归属：缺省共用、按应用记（不分大小写、会话关了还在）、没报应用名的按会话记，以及热加载时什么时候重置。
use crate::dispatch::{Router, RouterConfig};
use qingjian_core::Engine;
use qingjian_dictionary::Dictionary;
use qingjian_platform::protocol::{ClientMessage, PROTOCOL_VERSION, SessionId};
use serde_json::json;

fn router(config: RouterConfig) -> Router {
    Router::new(Engine::new(Dictionary::default()), config)
}

/// 两项都开：新应用从英文开始、各应用各记一份。
fn remembering() -> Router {
    router(RouterConfig {
        default_english: true,
        remember_mode_per_app: true,
        ..Default::default()
    })
}

fn open(router: &mut Router, id: u64, app: Option<&str>) {
    router.handle(ClientMessage::OpenSession {
        session: SessionId(id),
        app: app.map(str::to_owned),
        protocol: PROTOCOL_VERSION,
    });
}

/// 托盘切中 / 英（与单击切换键同一条路），返回回包带的新模式。
fn toggle(router: &mut Router, id: u64) -> bool {
    let reply = router
        .handle_linux(
            json!({"LinuxEvent": {"session": id, "event": {"Indicator": "ToggleEnglish"}}}),
        )
        .unwrap();
    reply["KeyResult"]["english"]
        .as_bool()
        .expect("切换的回包带 english")
}

fn english(router: &Router, id: u64) -> bool {
    router.english_of(SessionId(id))
}

#[test]
fn all_sessions_share_one_mode_by_default() {
    let mut router = router(RouterConfig::default());
    open(&mut router, 1, Some("foot"));
    open(&mut router, 2, Some("firefox"));
    assert!(!english(&router, 1));
    assert!(toggle(&mut router, 1));
    assert!(english(&router, 2), "切到别的应用也是英文");
    open(&mut router, 3, None);
    assert!(english(&router, 3), "新打开的也是英文");
    assert!(!toggle(&mut router, 3));
    assert!(!english(&router, 1));
}

#[test]
fn default_english_starts_the_shared_mode_in_english() {
    let mut router = router(RouterConfig {
        default_english: true,
        ..Default::default()
    });
    open(&mut router, 1, Some("foot"));
    assert!(english(&router, 1));
    // 关掉内置英文模式时始终是中文，默认英文不生效
    let mut router = self::router(RouterConfig {
        default_english: true,
        english_mode: false,
        ..Default::default()
    });
    open(&mut router, 1, Some("foot"));
    assert!(!english(&router, 1));
}

#[test]
fn apps_start_in_the_default_and_remember_independent_choices() {
    let mut router = remembering();
    open(&mut router, 1, Some("foot"));
    open(&mut router, 2, Some("org.gnome.TextEditor"));
    assert!(english(&router, 1));
    assert!(!toggle(&mut router, 1));
    assert!(english(&router, 2));
    assert!(!english(&router, 1));
    open(&mut router, 3, Some("firefox"));
    assert!(english(&router, 3));
}

#[test]
fn same_app_shares_its_mode_case_insensitively_and_survives_reopening() {
    let mut router = remembering();
    open(&mut router, 1, Some("Code"));
    assert!(!toggle(&mut router, 1));
    open(&mut router, 2, Some("code"));
    assert!(!english(&router, 2));
    router.handle(ClientMessage::CloseSession {
        session: SessionId(1),
    });
    router.handle(ClientMessage::CloseSession {
        session: SessionId(2),
    });
    open(&mut router, 3, Some("CODE"));
    assert!(!english(&router, 3));
    // 同一会话重开（前端重连），也不能盖掉应用的记忆
    open(&mut router, 3, Some("code"));
    assert!(!english(&router, 3));
}

/// 没报应用名（IBus 前端）或报了空串：每个会话各记一份，不共用一个占位名。
#[test]
fn sessions_without_an_app_keep_their_own_mode() {
    let mut router = remembering();
    open(&mut router, 1, None);
    open(&mut router, 2, None);
    open(&mut router, 3, Some(""));
    assert!(!toggle(&mut router, 1));
    assert!(english(&router, 2));
    assert!(english(&router, 3));
    assert!(!english(&router, 1));
}

#[test]
fn only_mode_policy_changes_reset_remembered_choices() {
    let mut router = remembering();
    open(&mut router, 1, Some("foot"));
    open(&mut router, 2, None);
    toggle(&mut router, 1);
    toggle(&mut router, 2);
    let reload = |router: &mut Router, next: &RouterConfig| {
        router.reload_mode(next);
        router.config = next.clone();
    };
    let mut next = router.config.clone();
    next.full_width = !next.full_width;
    reload(&mut router, &next);
    assert!(!english(&router, 1), "无关的配置改动不动记住的模式");
    assert!(!english(&router, 2));

    // 关掉内置英文模式时丢掉记住的；再打开从默认英文起
    next.english_mode = false;
    reload(&mut router, &next);
    assert!(!english(&router, 1));
    next.english_mode = true;
    reload(&mut router, &next);
    assert!(english(&router, 1));
    assert!(english(&router, 2));

    next.default_english = false;
    reload(&mut router, &next);
    assert!(!english(&router, 1));
    assert!(toggle(&mut router, 1));
    // 改成共用一份：从默认模式起，之前按应用记的不带过去
    next.remember_mode_per_app = false;
    reload(&mut router, &next);
    assert!(!english(&router, 1));
    assert!(!english(&router, 2));
}
