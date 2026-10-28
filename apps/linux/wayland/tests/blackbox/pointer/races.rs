//! 点击排队期间候选或菜单换了布局，旧落点不能落到新对象上。

use super::composing;
use crate::support::{Answer, BUTTON_LEFT, BUTTON_RIGHT, KEY_ESC, KEY_H, Request};

/// 点下去时排在前面的一个键正好换了帧：点击是按换帧之前的那一张认的，序号不能落到新的一帧上
/// （点的是「呢」，新帧第二个是「女孩」；新帧的展示身份 Server 照认，挡不住），这一下作废。
#[test]
fn a_click_queued_behind_a_new_frame_is_dropped() {
    let Some((rig, targets)) = composing(Answer::composing("n", &["你", "呢"]), &["Candidate"])
    else {
        return;
    };
    rig.server
        .answer(Answer::composing("nh", &["你好", "女孩"]));
    // 万一点击落到了新帧上，像真 Server 那样上屏新帧的第二个
    rig.server.answer(Answer::commit("女孩"));
    let (x, y) = targets.at("candidate1");
    rig.compositor.enter(x, y);
    let mark = rig.frontend.mark();
    let requests = rig.compositor.requests.len();
    rig.compositor.type_and_click(KEY_H, BUTTON_LEFT);
    assert!(
        rig.frontend.targets(mark).has("candidate1"),
        "新的一帧没贴上"
    );
    rig.compositor.sync();
    let clicks = rig.server.pointer_events("Candidate");
    assert!(
        clicks.is_empty(),
        "按旧布局认的点击落到了新帧上：{clicks:?}"
    );
    assert!(
        !rig.compositor
            .requests
            .since(requests)
            .iter()
            .any(|request| matches!(request, Request::CommitString(_))),
        "上屏了"
    );
}

#[test]
fn reopening_a_menu_drops_queued_old_items() {
    let Some((rig, targets)) = composing(Answer::composing("n", &["你", "呢"]), &["Menu"]) else {
        return;
    };
    let (x, y) = targets.at("candidate1");
    rig.compositor.enter(x, y);
    let mark = rig.frontend.mark();
    rig.compositor.click(BUTTON_RIGHT);
    let menu = rig.frontend.targets(mark);
    rig.compositor.sync();

    let (x, y) = targets.at("candidate0");
    let (item_x, item_y) = menu.at("item1");
    rig.compositor
        .clicks(&[(x, y, BUTTON_RIGHT), (item_x, item_y, BUTTON_LEFT)]);
    rig.compositor.sync();
    let menus = rig.server.pointer_events("Menu");
    assert_eq!(menus.len(), 2, "{menus:?}");
    assert_eq!(menus[1]["index"], 0);
    assert!(
        rig.server.pointer_events("MenuPick").is_empty(),
        "旧菜单项作用到了新候选"
    );
}

#[test]
fn queued_clicks_do_not_move_the_menu_anchor() {
    let Some((rig, targets)) = composing(Answer::composing("n", &["你", "呢"]), &["Menu"]) else {
        return;
    };
    let (x, y) = targets.at("candidate0");
    rig.compositor.enter(x, y);
    let mark = rig.frontend.mark();
    rig.compositor.click(BUTTON_RIGHT);
    let expected = rig.frontend.targets(mark).at("item0");
    rig.compositor.tap(KEY_ESC);
    rig.compositor.sync();

    let (later_x, later_y) = targets.at("candidate1");
    let mark = rig.frontend.mark();
    rig.compositor
        .clicks(&[(x, y, BUTTON_RIGHT), (later_x, later_y, BUTTON_LEFT)]);
    assert_eq!(
        rig.frontend.targets(mark).at("item0"),
        expected,
        "菜单应在这次右键处弹出，不能取后续点击的位置"
    );
    rig.compositor.sync();
}
