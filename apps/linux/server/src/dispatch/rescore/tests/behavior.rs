//! 知意沿用停键重排，并且不改用户已移动高亮的页。
use super::prefers::Prefers;
use super::{attach, compose, router};
use crate::dispatch::Router;
use qingjian_platform::protocol::{
    ClientMessage, KeyEvent, KeyModifiers, ServerMessage, SessionId,
};
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

fn first(router: &mut Router) -> String {
    let Some(ServerMessage::Update { frame, .. }) = router.handle(ClientMessage::Poll {
        session: SessionId(1),
    }) else {
        panic!("应有候选帧")
    };
    frame.candidates.items[0].text.clone()
}

#[test]
fn choice_model_attached_mid_composition_reranks_after_pause() {
    let mut router = router();
    compose(&mut router);
    assert_eq!(first(&mut router), "你他");
    attach(&mut router, &Arc::new(Prefers::default()));
    assert!(router.next_tick() <= Duration::from_millis(80));
    let deadline = Instant::now() + Duration::from_secs(3);
    while first(&mut router) != "你它" {
        assert!(Instant::now() < deadline, "选词模型未换首选");
        std::thread::sleep(Duration::from_millis(10));
        router.tick();
    }
}

#[test]
fn choice_model_does_not_replace_a_navigated_page() {
    let mut router = router();
    attach(&mut router, &Arc::new(Prefers::default()));
    compose(&mut router);
    router.handle(ClientMessage::Key {
        session: SessionId(1),
        event: KeyEvent::new(0x28, None, KeyModifiers::default()),
    });
    std::thread::sleep(Duration::from_millis(90));
    let deadline = Instant::now() + Duration::from_secs(3);
    while router.next_tick() < Duration::from_secs(1) {
        assert!(Instant::now() < deadline);
        router.tick();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(first(&mut router), "你他");
}

/// Linux 事件给的应用前文也传到知意选词器，私密输入始终不传。
#[test]
fn choice_model_reads_application_context_only_outside_sensitive_fields() {
    let before = "应用里已有的光标前文";
    for sensitive in [false, true] {
        let mut router = router();
        router.handle_linux(json!({"LinuxEvent": {"session": 1, "event": {
            "Capabilities": {"sensitive": sensitive, "password": false, "disabled": false}
        }}}));
        let model = Arc::new(Prefers::default());
        model.context_chars.store(usize::MAX, Ordering::SeqCst);
        attach(&mut router, &model);
        for c in "nita".chars() {
            router.handle_linux(json!({"LinuxEvent": {"session": 1, "event": {"Key": {
                "event": KeyEvent::new(c as u32, Some(c), KeyModifiers::default()),
                "release": false, "surrounding": {"before": before, "after": ""}
            }}}}));
        }
        let expected = if sensitive { 0 } else { before.chars().count() };
        let deadline = Instant::now() + Duration::from_secs(3);
        while model.context_chars.load(Ordering::SeqCst) != expected {
            assert!(Instant::now() < deadline, "光标前文未按隐私能力传入选词器");
            router.tick();
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
