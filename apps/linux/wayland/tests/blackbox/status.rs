//! 中 / 英提示与候选、右键菜单共用 popup，缓冲背压后仍须贴出当前显示内容。

use std::time::Duration;

use qingjian_platform::protocol::linux::InputMode;

use crate::pointer::composing;
use crate::support::{Answer, BUTTON_LEFT, BUTTON_RIGHT, KEY_H, KEY_N, Picture, Request, Rig};

fn english() -> InputMode {
    InputMode {
        english: true,
        ..InputMode::default()
    }
}

fn latest_picture(rig: &Rig) -> Picture {
    rig.compositor
        .requests
        .entries()
        .iter()
        .rev()
        .find_map(|request| match request {
            Request::Popup {
                picture: Some(picture),
                ..
            } => Some(*picture),
            _ => None,
        })
        .expect("候选已贴上")
}

fn hint_is_painted(request: &Request, candidate: Picture) -> bool {
    matches!(request, Request::Popup { picture: Some(picture), .. }
        if picture.painted && picture.width < candidate.width && picture.height < candidate.height)
}

/// 候选换成提示，再换回候选并弹菜单；旧提示的定时器不能收掉新的候选和菜单。
#[test]
fn candidates_and_the_menu_take_over_the_mode_hint() {
    let Some((rig, _)) = composing(Answer::composing("n", &["你", "呢"]), &["Menu"]) else {
        return;
    };
    rig.compositor.sync();
    let candidate = latest_picture(&rig);
    rig.server.answer(Answer::commit("n").switched(english()));
    let mark = rig.compositor.requests.len();
    rig.compositor.tap(KEY_H);
    rig.compositor
        .requests
        .wait(mark, "中 / 英提示贴上", |request| {
            hint_is_painted(request, candidate)
        });
    rig.compositor.sync();

    rig.server.answer(Answer::composing("h", &["hello", "hi"]));
    let mark = rig.frontend.mark();
    rig.compositor.tap(KEY_N);
    let targets = rig.frontend.targets(mark);
    let (x, y) = targets.at("candidate1");
    rig.compositor.enter(x, y);
    let mark = rig.frontend.mark();
    rig.compositor.click(BUTTON_RIGHT);
    assert!(
        rig.frontend.targets(mark).has("item0"),
        "提示之后的菜单未弹出"
    );
    let mark = rig.compositor.requests.len();
    assert!(
        rig.compositor
            .requests
            .wait_for(mark, Duration::from_millis(1000), |request| matches!(
                request,
                Request::Popup { picture: None, .. }
            ))
            .is_none(),
        "旧提示定时器收掉了菜单"
    );
}

/// 所有缓冲都被暂扣时切换模式，归还后必须补贴提示，不能恢复旧候选或旧命中区域。
#[test]
fn buffer_release_paints_the_hint_instead_of_old_candidates() {
    let Some((rig, targets)) = composing(Answer::composing("n", &["你", "呢"]), &["Menu"]) else {
        return;
    };
    rig.compositor.sync();
    let candidate = latest_picture(&rig);
    rig.compositor.hold_buffers(true);
    for text in ["你们", "你们好", "你好啊", "最新候选"] {
        rig.server
            .answer(Answer::composing(text, &[text, "第二项"]));
        rig.compositor.tap(KEY_H);
        rig.compositor.sync();
    }
    assert!(
        rig.frontend.logged("缓冲都还在合成器手里"),
        "没有覆盖缓冲背压"
    );
    rig.server.answer(Answer::commit("n").switched(english()));
    rig.compositor.tap(KEY_H);
    rig.compositor.sync();
    let mark = rig.compositor.requests.len();
    rig.compositor.hold_buffers(false);
    rig.compositor
        .requests
        .wait(mark, "归还缓冲后补贴提示", |request| {
            hint_is_painted(request, candidate)
        });

    let (x, y) = targets.at("candidate0");
    rig.compositor.enter(x, y);
    rig.compositor.click(BUTTON_LEFT);
    rig.compositor.click(BUTTON_RIGHT);
    rig.compositor.sync();
    assert!(rig.server.pointer_events("Candidate").is_empty());
    assert!(
        rig.server.pointer_events("Menu").is_empty(),
        "提示沿用了旧候选落点"
    );
}
