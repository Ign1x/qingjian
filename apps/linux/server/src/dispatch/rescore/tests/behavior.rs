//! 知意沿用停键重排，并且不改用户已移动高亮的页。
use super::prefers::Prefers;
use super::{attach, compose, router};
use crate::dispatch::Router;
use qingjian_platform::protocol::{
    ClientMessage, KeyEvent, KeyModifiers, ServerMessage, SessionId,
};
use std::sync::Arc;
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
