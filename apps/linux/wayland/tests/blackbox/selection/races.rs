//! 在 Server 回包前同步送出合成器更新，不靠线程睡眠碰时序。

use crate::support::{Answer, Field, KEY_SPACE, KEY_T, Request, Rig, Setup, kind, text_updates};

#[test]
fn changing_the_field_while_requesting_selection_sends_no_text() {
    for field in [
        Field::password(),
        Field::text("我喜欢苹果。", 15, 9).sensitive(),
        Field::text("我喜欢苹果。", 6, 0).by_app(),
        // 即使原因是 input_method，也不能把旧选区当成最新选区交出去。
        Field::text("我喜欢苹果。", 6, 0),
    ] {
        let Some(rig) = Rig::start(Setup::default()) else {
            return;
        };
        rig.focus_with(Field::text("我喜欢苹果。", 15, 9));
        rig.server.answer(Answer::select(7));
        rig.server
            .before_reply("Key", rig.compositor.update_before_reply(field));
        rig.compositor.tap(KEY_T);
        rig.server.received.wait(0, "拒绝读取过期选区", |message| {
            kind(message) == "Selection"
        });
        rig.compositor.sync();
        assert_eq!(rig.server.selections(), [""]);
    }
}

#[test]
fn moving_the_selection_while_committing_never_inserts_the_translation() {
    let Some(mut rig) = Rig::start(Setup::default()) else {
        return;
    };
    rig.focus_with(Field::text("我喜欢苹果。", 15, 9));
    rig.server.answer(Answer::select(7));
    rig.compositor.tap(KEY_T);
    rig.compositor.sync();
    let reported = rig.server.received.len();
    rig.server.translated("apple");
    rig.server
        .received
        .wait(reported, "译文已经显示", |message| {
            message.get("DisplayAcknowledged").is_some()
        });
    let mark = rig.compositor.requests.len();
    rig.server.answer(Answer::commit("apple"));
    rig.server.before_reply(
        "Key",
        rig.compositor
            .update_before_reply(Field::text("我喜欢苹果。", 6, 0).by_app()),
    );
    rig.compositor.tap(KEY_SPACE);
    rig.compositor.sync();
    rig.stop();
    assert!(
        !text_updates(&rig.compositor.requests.since(mark))
            .iter()
            .any(|request| matches!(request, Request::CommitString(_))),
        "旧译文不能覆盖新选区"
    );
    assert!(
        rig.server
            .received
            .entries()
            .iter()
            .any(|message| kind(message) == "Reset")
    );
}
