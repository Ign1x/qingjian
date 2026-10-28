//! 合成器暂扣全部缓冲时不按旧画面接受点击，归还后补贴最新画面。

use super::composing;
use crate::support::{Answer, BUTTON_LEFT, BUTTON_RIGHT, KEY_H, Request};

#[test]
fn buffer_release_restores_the_latest_click_targets() {
    let Some((rig, targets)) = composing(Answer::composing("n", &["你", "呢"]), &["Candidate"])
    else {
        return;
    };
    rig.compositor.sync();
    rig.compositor.hold_buffers(true);
    for text in ["你们", "你们好", "你好啊", "最新的候选"] {
        rig.server
            .answer(Answer::composing(text, &[text, "新的第二项"]));
        rig.compositor.tap(KEY_H);
        rig.compositor.sync();
    }
    assert!(
        rig.frontend.logged("缓冲都还在合成器手里"),
        "没有覆盖贴图失败路径"
    );
    let (x, y) = targets.at("candidate0");
    rig.compositor.enter(x, y);
    rig.compositor.click(BUTTON_LEFT);
    rig.compositor.sync();
    assert!(
        rig.server.pointer_events("Candidate").is_empty(),
        "新帧没贴上时仍收了点击"
    );

    let mark = rig.frontend.mark();
    rig.compositor.hold_buffers(false);
    let targets = rig.frontend.targets(mark);
    let (x, y) = targets.at("candidate1");
    rig.server.answer(Answer::commit("新的第二项"));
    let mark = rig.compositor.requests.len();
    rig.compositor.motion(x, y);
    rig.compositor.click(BUTTON_LEFT);
    rig.compositor
        .requests
        .wait(mark, "归还缓冲后点选上屏", |request| {
            *request == Request::CommitString("新的第二项".to_owned())
        });
    assert_eq!(rig.server.pointer_events("Candidate")[0]["index"], 1);
}

#[test]
fn an_unpainted_hover_does_not_turn_a_click_into_menu_dismissal() {
    let Some((rig, targets)) = composing(Answer::composing("n", &["你", "呢"]), &["Menu"]) else {
        return;
    };
    let (x, y) = targets.at("candidate0");
    rig.compositor.enter(x, y);
    let mark = rig.frontend.mark();
    rig.compositor.click(BUTTON_RIGHT);
    let menu = rig.frontend.targets(mark);
    rig.compositor.sync();
    rig.compositor.hold_buffers(true);
    for name in ["item1", "item0", "item1", "item0"] {
        let (x, y) = menu.at(name);
        rig.compositor.motion(x, y);
        rig.compositor.sync();
    }
    assert!(rig.frontend.logged("缓冲都还在合成器手里"));
    rig.compositor.click(BUTTON_LEFT);
    rig.compositor.sync();
    assert!(rig.server.pointer_events("MenuPick").is_empty());

    let mark = rig.frontend.mark();
    rig.compositor.hold_buffers(false);
    assert!(
        rig.frontend.targets(mark).has("item0"),
        "无有效画面时的点击误关了菜单"
    );
}
