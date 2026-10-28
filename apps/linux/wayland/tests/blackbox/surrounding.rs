//! 前后文：按下带上光标两侧的文字；组句中应用自己改了文字（text_change_cause 是 other）就丢掉组句。

use super::support::{
    Answer, Field, KEY_A, KEY_N, KEY_SPACE, Request, Rig, Setup, kind, text_updates,
};

/// 应用报了前后文：按下带上光标两侧的文字，抬起不带；光标处没有选区，不报能读选区。
#[test]
fn presses_carry_the_text_around_the_cursor() {
    let Some(rig) = Rig::start(Setup::default()) else {
        return;
    };
    rig.focus_with(Field::text("今天天气", 6, 6));
    let mark = rig.compositor.requests.len();
    rig.compositor.tap(KEY_A);
    rig.compositor
        .requests
        .wait(mark, "转给应用的抬起", |request| {
            matches!(request, Request::Key { pressed: false, .. })
        });
    let reports = rig.server.key_reports();
    assert_eq!(reports.len(), 2, "{reports:#?}");
    let surrounding = reports[0].surrounding.as_ref().expect("按下带着前后文");
    assert_eq!(
        (surrounding.before.as_str(), surrounding.after.as_str()),
        ("今天", "天气")
    );
    assert!(!reports[0].selection_supported);
    assert!(reports[1].release && reports[1].surrounding.is_none());
}

/// 组句中应用照上屏改了文字（原因是 input_method）：不算被改，组句照旧；
/// 应用自己改了文字（光标挪走、自动更正）：报 Reset，清掉行内拼音，同一次 commit。
#[test]
fn an_edit_by_the_app_drops_the_composition() {
    let Some(rig) = Rig::start(Setup::default()) else {
        return;
    };
    rig.focus_with(Field::text("今天", 6, 6));
    rig.server
        .answer(Answer::composing("nihao", &["你好", "你"]));
    rig.server.answer(Answer {
        commit: Some("你".to_owned()),
        ..Answer::composing("hao", &["好"])
    });
    let mark = rig.compositor.requests.len();
    rig.compositor.tap(KEY_N);
    rig.compositor.requests.wait(mark, "行内拼音", |request| {
        matches!(request, Request::Commit(_))
    });
    // 应用落定了这次更新：下一次更新不用等满 80 毫秒
    rig.compositor.done();
    let mark = rig.compositor.requests.len();
    rig.compositor.tap(KEY_SPACE);
    rig.compositor
        .requests
        .wait(mark, "上屏一个字、留着 hao", |request| {
            matches!(request, Request::Commit(_))
        });

    let reported = rig.server.received.len();
    let mark = rig.compositor.requests.len();
    rig.compositor.update(Field::text("今天你", 9, 9));
    rig.compositor.sync();
    assert!(
        rig.server
            .received
            .since(reported)
            .iter()
            .all(|message| kind(message) != "Reset"),
        "输入法上屏引起的变化当成了应用改的"
    );
    let updates = text_updates(&rig.compositor.requests.since(mark));
    assert!(updates.is_empty(), "{updates:#?}");

    let mark = rig.compositor.requests.len();
    rig.compositor
        .update(Field::text("今天你。", 12, 12).by_app());
    rig.server
        .received
        .wait(reported, "Reset", |message| kind(message) == "Reset");
    rig.compositor
        .requests
        .wait(mark, "清掉行内拼音", |request| {
            matches!(request, Request::Commit(_))
        });
    assert_eq!(
        text_updates(&rig.compositor.requests.since(mark)),
        [
            Request::Preedit {
                text: String::new(),
                begin: 0,
                end: 0,
            },
            Request::Commit(rig.compositor.dones()),
        ]
    );
    if rig.frontend.painter() {
        rig.compositor.requests.wait(mark, "候选窗收起", |request| {
            matches!(request, Request::Popup { picture: None, .. })
        });
    }
}
