//! 回包解析：Linux Server 在共用消息上另附的字段，以及只有 Linux 才有的几种回包。

use serde_json::json;

use crate::protocol::linux::client::{ClientError, Reply};
use crate::protocol::linux::{InputMode, MenuItem};
use crate::protocol::{Frame, KeyOutcome};

/// 回包上 Linux Server 另附的几样：展示身份整份读出来、切换中 / 英时的 `english`、帧上的 `translation_pending`、模式。
#[test]
fn reply_reads_linux_extras() {
    let mut frame = serde_json::to_value(Frame::default()).expect("帧");
    frame["translation_pending"] = json!(true);
    let reply = Reply::from_value(json!({"KeyResult": {
        "session": 1,
        "outcome": KeyOutcome::Passthrough,
        "commit": "nihao",
        "frame": frame,
        "english": true,
        "identity": {"generation": 3, "context": "wayland", "revision": 9},
        "mode": {"english": true, "traditional": true, "full_width": false, "scheme": "小鹤双拼"},
    }}))
    .expect("合法回包");
    assert_eq!(reply.commit.as_deref(), Some("nihao"));
    assert_eq!(reply.english, Some(true));
    assert!(reply.translation_pending);
    let identity = reply.identity.expect("身份");
    assert_eq!(
        (
            identity.generation,
            identity.context.as_str(),
            identity.revision
        ),
        (3, "wayland", 9)
    );
    let mode = reply.mode.expect("模式");
    assert_eq!(
        mode,
        InputMode {
            english: true,
            traditional: true,
            full_width: false,
            scheme: "小鹤双拼".to_owned(),
        }
    );
    assert_eq!(mode.scheme_name(), "小鹤双拼");
}

/// 轮询的 `Update` 也带模式；老 Server 不带时是 `None`，前端照旧能用。
#[test]
fn mode_rides_on_updates_and_is_optional() {
    let polled = Reply::from_value(json!({"Update": {
        "session": 1,
        "frame": Frame::default(),
        "mode": {"english": false, "traditional": false, "full_width": true, "scheme": ""},
    }}))
    .expect("轮询回包");
    let mode = polled.mode.expect("模式");
    assert!(mode.full_width);
    assert_eq!(
        mode.scheme_name(),
        "全拼",
        "单开全拼时状态条不写方案，主题要写出来"
    );

    let old = Reply::from_value(json!({"KeyResult": {
        "session": 1,
        "outcome": KeyOutcome::Consumed,
        "commit": null,
        "frame": Frame::default(),
    }}))
    .expect("老 Server 的回包");
    assert_eq!(old.mode, None);
    assert_eq!(old.outcome, KeyOutcome::Consumed);
}

/// 右键菜单的回包读成 `menu`；按键与上屏都是缺省（放行、不上屏），前端只拿它弹菜单。
#[test]
fn menu_reply_lists_items() {
    let reply = Reply::from_value(json!({"Menu": {"session": 1, "items": [
        {"id": 1, "label": "置顶"},
        {"id": 5, "label": "删除"},
    ]}}))
    .expect("菜单回包");
    assert_eq!(
        reply.menu,
        [
            MenuItem {
                id: 1,
                label: "置顶".to_owned()
            },
            MenuItem {
                id: 5,
                label: "删除".to_owned()
            },
        ]
    );
    assert_eq!(reply.outcome, KeyOutcome::Passthrough);
    assert!(reply.commit.is_none());
    assert!(matches!(
        Reply::from_value(json!({"Menu": {"session": 1}})),
        Err(ClientError::Unexpected(_))
    ));
}

/// `RequestSelection` 读成请求编号：按键先当放行，真正吃不吃看回了选区之后的那次回包。
#[test]
fn selection_request_carries_its_number() {
    let reply = Reply::from_value(json!({"RequestSelection": {"session": 1, "request": 4}}))
        .expect("选区请求");
    assert_eq!(reply.selection_request, Some(4));
    assert_eq!(reply.outcome, KeyOutcome::Passthrough);
    assert!(reply.menu.is_empty());
}

/// `Ignored` 当没事发生（放行）；`Committed` 这类 Linux 事件不会收到的回包报意外，不当成按键结果。
#[test]
fn reply_rejects_what_it_does_not_handle() {
    let ignored = Reply::from_value(json!({"Ignored": {"session": 1}})).expect("Ignored");
    assert_eq!(ignored.outcome, KeyOutcome::Passthrough);
    assert!(ignored.identity.is_none());
    assert!(ignored.mode.is_none());
    let committed = Reply::from_value(json!({"Committed": {"session": 1, "text": "ni"}}));
    assert!(matches!(committed, Err(ClientError::Unexpected(_))));
}
