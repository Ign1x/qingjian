//! 选区译文也能鼠标点选；鼠标请求等待期间选区或隐私变化时，旧译文不能上屏。

use super::translation::press_translate;
use crate::support::{
    Answer, BUTTON_LEFT, Field, Request, Rig, Setup, Targets, kind, skip, text_updates,
};

fn translated() -> Option<(Rig, Targets)> {
    let rig = Rig::start(Setup::default())?;
    if !rig.frontend.painter() {
        skip("这台机器上没有界面字体，候选窗画不出来");
        return None;
    }
    rig.server.allow(&["Candidate"]);
    rig.focus_with(Field::text("我喜欢苹果。", 15, 9));
    let mark = rig.frontend.mark();
    press_translate(&rig, Answer::select(7));
    let targets = rig.frontend.targets(mark);
    // 短译文沿用同一落点，落点没变不重复记日志；展示回执确认译文那一帧已显示。
    let reported = rig.server.received.len();
    rig.server.translated("apple");
    rig.server.received.wait(reported, "译文已显示", |message| {
        message.get("DisplayAcknowledged").is_some()
    });
    rig.compositor.sync();
    Some((rig, targets))
}

#[test]
fn clicking_the_translation_replaces_the_selection() {
    let Some((rig, targets)) = translated() else {
        return;
    };
    rig.server.answer(Answer::commit("apple"));
    let mark = rig.compositor.requests.len();
    let (x, y) = targets.at("candidate0");
    rig.compositor.enter(x, y);
    rig.compositor.click(BUTTON_LEFT);
    rig.compositor
        .requests
        .wait(mark, "点选译文上屏", |request| {
            *request == Request::CommitString("apple".to_owned())
        });
    rig.compositor.sync();
    assert_eq!(rig.server.selections(), ["苹果"]);
    assert_eq!(rig.server.pointer_events("Candidate").len(), 1);
}

#[test]
fn changing_the_field_while_clicking_never_inserts_the_translation() {
    for field in [
        Field::text("我喜欢苹果。", 15, 9).sensitive(),
        Field::text("我喜欢苹果。", 6, 0).by_app(),
    ] {
        let Some((mut rig, targets)) = translated() else {
            return;
        };
        rig.server.answer(Answer::commit("apple"));
        rig.server
            .before_reply("Candidate", rig.compositor.update_before_reply(field));
        let mark = rig.compositor.requests.len();
        let (x, y) = targets.at("candidate0");
        rig.compositor.enter(x, y);
        rig.compositor.click(BUTTON_LEFT);
        rig.server
            .received
            .wait(0, "已点选译文", |message| kind(message) == "Candidate");
        rig.compositor.sync();
        rig.stop();
        assert!(
            !text_updates(&rig.compositor.requests.since(mark))
                .iter()
                .any(|request| matches!(request, Request::CommitString(_))),
            "输入目标已变，旧译文不能上屏"
        );
    }
}
