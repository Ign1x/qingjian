//! 选区翻译：应用报的前后文里有选区，按 Alt+Shift+T（Linux 的缺省翻译快捷键），Server 要选区、前端回选中的文字，
//! 「翻译中…」时接着轮询，译文到了换帧，空格用译文替换选区；读不了的选区回空串，快捷键原样交给应用。

use serde_json::json;

use super::support::{
    ALT, Answer, Field, KEY_LEFTALT, KEY_LEFTSHIFT, KEY_SPACE, KEY_T, Picture, Request, Rig, SHIFT,
    Setup, kind, text_updates,
};

/// 按一次 Alt+Shift+T：Alt、Shift 本身放行（转给应用），T 照 `answer` 答；最后等被测进程处理完这一串。
fn press_translate(rig: &Rig, answer: Answer) {
    rig.server.answer(Answer::passthrough());
    rig.server.answer(Answer::passthrough());
    rig.server.answer(answer);
    rig.compositor.press_modifier(KEY_LEFTALT, ALT);
    rig.compositor.press_modifier(KEY_LEFTSHIFT, SHIFT);
    rig.compositor.tap(KEY_T);
    rig.compositor.release_modifier(KEY_LEFTSHIFT, SHIFT);
    rig.compositor.release_modifier(KEY_LEFTALT, ALT);
    rig.compositor.sync();
}

/// 转给应用的键：（键码，按下还是抬起）。
fn forwarded(requests: &[Request]) -> Vec<(u32, bool)> {
    requests
        .iter()
        .filter_map(|request| match request {
            Request::Key { code, pressed, .. } => Some((*code, *pressed)),
            _ => None,
        })
        .collect()
}

/// 一整轮：T 带着能读选区与选区两侧的文字报上去，Server 要选区就回「苹果」；「翻译中…」时不往应用里放东西，接着轮询，
/// 译文到了换帧、回执；空格把译文上屏（应用拿它替换选区）。T 与空格吃掉，只有 Alt、Shift 转给应用；选中的文字与译文不进日志。
#[test]
fn the_selection_is_translated_and_replaced() {
    let Some(mut rig) = Rig::start(Setup::default()) else {
        return;
    };
    rig.focus_with(Field::text("我喜欢苹果。", 15, 9));
    let mark = rig.compositor.requests.len();
    press_translate(&rig, Answer::select(7));
    let selection = rig
        .server
        .received
        .wait(0, "回选区", |message| kind(message) == "Selection");
    assert_eq!(
        selection["LinuxEvent"]["event"]["Selection"],
        json!({"request": 7, "text": "苹果"})
    );
    let reports = rig.server.key_reports();
    let t = reports
        .iter()
        .find(|report| !report.release && report.event.character == Some('T'))
        .expect("T 报给了 Server");
    assert!(t.selection_supported);
    assert!(t.event.modifiers.alt && t.event.modifiers.shift);
    let surrounding = t.surrounding.as_ref().expect("带着前后文");
    assert_eq!(
        (surrounding.before.as_str(), surrounding.after.as_str()),
        ("我喜欢", "。"),
        "选区跳过"
    );
    rig.server.received.wait(0, "等译文时的轮询", |message| {
        kind(message) == "Poll"
    });
    assert!(
        text_updates(&rig.compositor.requests.since(mark)).is_empty(),
        "「翻译中…」没有拼音，不该动应用里的文字（选区要留着）"
    );

    // 按键都处理完了：之后的展示回执只会来自译文那一帧（轮询回来帧没变不重画、不回执）
    let reported = rig.server.received.len();
    let drawn = rig.compositor.requests.len();
    rig.server.translated("apple");
    rig.server
        .received
        .wait(reported, "译文那一帧的展示回执", |message| {
            message.get("DisplayAcknowledged").is_some()
        });
    if rig.frontend.painter() {
        rig.compositor.requests.wait(drawn, "画上译文", |request| {
            matches!(
                request,
                Request::Popup {
                    picture: Some(Picture { painted: true, .. }),
                    ..
                }
            )
        });
    }

    rig.server.answer(Answer::commit("apple"));
    let mark = rig.compositor.requests.len();
    rig.compositor.tap(KEY_SPACE);
    rig.compositor.requests.wait(mark, "上屏译文", |request| {
        matches!(request, Request::Commit(_))
    });
    assert_eq!(
        text_updates(&rig.compositor.requests.since(mark)),
        [
            Request::CommitString("apple".to_owned()),
            Request::Preedit {
                text: String::new(),
                begin: 0,
                end: 0,
            },
            Request::Commit(rig.compositor.dones()),
        ]
    );
    rig.compositor.sync();
    let codes: Vec<u32> = forwarded(&rig.compositor.requests.entries())
        .into_iter()
        .map(|(code, _)| code)
        .collect();
    assert!(
        codes
            .iter()
            .all(|code| [KEY_LEFTALT, KEY_LEFTSHIFT].contains(code)),
        "T 与空格吃掉了，不该转给应用：{codes:?}"
    );

    rig.stop();
    let log = rig.frontend.log().join("\n");
    for secret in ["苹果", "喜欢", "apple"] {
        assert!(!log.contains(secret), "日志里有「{secret}」：\n{log}");
    }
}

/// 选中了 600 个字（超过 500 字）：回空串，Server 放行，Alt+Shift+T 原样交给应用，候选窗不出来。
#[test]
fn a_long_selection_hands_the_shortcut_to_the_app() {
    let Some(rig) = Rig::start(Setup::default()) else {
        return;
    };
    let long = "字".repeat(600);
    rig.focus_with(Field::text(&long, long.len(), 0));
    let mark = rig.compositor.requests.len();
    press_translate(&rig, Answer::select(8));
    assert_eq!(rig.server.selections(), [""]);
    let requests = rig.compositor.requests.since(mark);
    assert_eq!(
        forwarded(&requests),
        [
            (KEY_LEFTALT, true),
            (KEY_LEFTSHIFT, true),
            (KEY_T, true),
            (KEY_T, false),
            (KEY_LEFTSHIFT, false),
            (KEY_LEFTALT, false),
        ]
    );
    assert!(
        !requests
            .iter()
            .any(|request| matches!(request, Request::Popup { .. })),
        "没在翻译，候选窗不该出来"
    );
}
