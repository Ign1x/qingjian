//! 点选、翻页、右键菜单、托盘切换与选区翻译：每个调用发出去的都是 Server 认得的那种 `LinuxEvent`。

use serde_json::{Value, json};

use super::fake_server;
use crate::protocol::linux::client::Connection;
use crate::protocol::linux::key::to_key_event;
use crate::protocol::linux::{Capabilities, DisplayIdentity, LinuxEvent, ModeCommand};
use crate::protocol::{KeyOutcome, SessionId};

fn identity() -> DisplayIdentity {
    DisplayIdentity {
        generation: 1,
        context: "/ibus/context/1".to_owned(),
        revision: 7,
    }
}

/// 收到的第 `index` 条消息里的 Linux 事件，按 Server 那边的类型解出来。
fn event(seen: &[Value], index: usize, session: SessionId) -> LinuxEvent {
    let message = &seen[index]["LinuxEvent"];
    assert_eq!(message["session"], json!(session), "{message}");
    serde_json::from_value(message["event"].clone()).expect("Server 解得出这个事件")
}

#[test]
fn panel_calls_carry_the_display_identity() {
    let (path, received, server) = fake_server(None);
    let mut connection = Connection::connect_at(&path).expect("连上假 Server");
    let session = connection
        .open_session("/ibus/context/1", None)
        .expect("开会话");
    connection
        .capabilities(session, Capabilities::default())
        .expect("报能力");

    let clicked = connection
        .candidate(session, identity(), 2)
        .expect("点候选");
    assert_eq!(clicked.outcome, KeyOutcome::Consumed);
    connection.page(session, identity(), true).expect("翻页");
    let menu = connection.menu(session, identity(), 1).expect("右键");
    let labels: Vec<_> = menu.menu.iter().map(|item| item.label.as_str()).collect();
    assert_eq!(labels, ["降权", "删除"]);
    assert!(menu.identity.is_none(), "问菜单不换帧，没有新的展示身份");
    connection
        .menu_pick(session, identity(), 1, menu.menu[0].id)
        .expect("点菜单项");

    connection.close_session(session).expect("关会话");
    drop(connection);
    server.join().expect("假 Server 收工");
    let _ = std::fs::remove_file(&path);

    let seen = received.lock().expect("取回收到的消息");
    assert!(matches!(
        event(&seen, 3, session),
        LinuxEvent::Candidate { identity: shown, index: 2 } if shown == identity()
    ));
    assert!(matches!(
        event(&seen, 4, session),
        LinuxEvent::Page { next: true, .. }
    ));
    assert!(matches!(
        event(&seen, 5, session),
        LinuxEvent::Menu { identity: shown, index: 1 } if shown == identity()
    ));
    assert!(matches!(
        event(&seen, 6, session),
        LinuxEvent::MenuPick {
            index: 1,
            action: 3,
            ..
        }
    ));
}

/// 托盘切换发的是 `Indicator`，回包像按键一样落地、附上切完的模式。
#[test]
fn indicator_sends_the_mode_command() {
    let (path, received, server) = fake_server(None);
    let mut connection = Connection::connect_at(&path).expect("连上假 Server");
    let session = connection
        .open_session("/ibus/context/1", None)
        .expect("开会话");
    connection
        .capabilities(session, Capabilities::default())
        .expect("报能力");
    let toggled = connection
        .indicator(session, ModeCommand::ToggleTraditional)
        .expect("托盘切简繁");
    assert!(toggled.mode.is_some());

    drop(connection);
    server.join().expect("假 Server 收工");
    let _ = std::fs::remove_file(&path);
    let seen = received.lock().expect("取回收到的消息");
    assert!(matches!(
        event(&seen, 3, session),
        LinuxEvent::Indicator(ModeCommand::ToggleTraditional)
    ));
}

/// 选区翻译：按键报能读选区，Server 回请求编号；前端读出选区用同一个编号回，那次的回包才是这一键的结果。
#[test]
fn selection_request_is_answered_with_the_same_number() {
    let (path, received, server) = fake_server(None);
    let mut connection = Connection::connect_at(&path).expect("连上假 Server");
    let session = connection
        .open_session("/ibus/context/1", None)
        .expect("开会话");
    connection
        .capabilities(session, Capabilities::default())
        .expect("报能力");

    // Alt+Shift+T：X11 掩码里 Shift 是第 0 位、Alt（Mod1）是第 3 位
    let shortcut = to_key_event('T' as u32, 1 | 1 << 3);
    let asked = connection
        .key_flushing(session, shortcut, false, None, true)
        .expect("按选区翻译键");
    let request = asked.selection_request.expect("Server 要选区");
    let answered = connection
        .selection(session, request, "你好".to_owned())
        .expect("回选区");
    assert_eq!(answered.outcome, KeyOutcome::Consumed);
    assert_eq!(answered.selection_request, None);

    drop(connection);
    server.join().expect("假 Server 收工");
    let _ = std::fs::remove_file(&path);
    let seen = received.lock().expect("取回收到的消息");
    assert!(matches!(
        event(&seen, 3, session),
        LinuxEvent::Key {
            selection_supported: true,
            release: false,
            ..
        }
    ));
    match event(&seen, 4, session) {
        LinuxEvent::Selection { request: 4, text } => assert_eq!(text, "你好"),
        other => panic!("回选区要发 Selection，实际是 {other:?}"),
    }
}
