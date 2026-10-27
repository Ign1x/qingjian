//! 用两个 Rust 前端共用的客户端（`qingjian_platform::protocol::linux::client`）对着真 Server 走一遍：
//! 握手、按键往返、轮询的版本号、展示回执、快捷键冲洗、Shift 单击切换、失焦不上屏。
#![cfg(target_os = "linux")]
#[path = "support/server.rs"]
mod server;

use qingjian_platform::protocol::KeyOutcome;
use qingjian_platform::protocol::linux::client::Connection;
use qingjian_platform::protocol::linux::key::{keysym, to_key_event};
use qingjian_platform::protocol::linux::{AppSurrounding, Capabilities};
use server::Server;

/// X11 修饰键掩码的 Control 位。
const CONTROL: u32 = 1 << 2;

#[test]
fn shared_client_round_trips_against_the_real_server() {
    let directory = std::env::temp_dir().join(format!("qingjian-client-{}", std::process::id()));
    std::fs::create_dir_all(directory.join("config/qingjian")).unwrap();
    std::fs::write(
        directory.join("config/qingjian/config.toml"),
        "[shortcut]\nswitch_mode = \"shift\"\n",
    )
    .unwrap();
    let mut server = Server::start(directory.clone());
    drop(server.connect());
    let mut connection = Connection::connect_at(&directory.join("server.sock")).unwrap();

    let session = connection.open_session("client-test").unwrap();
    connection
        .capabilities(session, Capabilities::default())
        .unwrap();
    connection.focus(session, true).unwrap();

    let surrounding = AppSurrounding::around("今天", 6, 6);
    let mut shown = 0;
    let mut last = None;
    for character in "nihao".chars() {
        let reply = connection
            .key(
                session,
                to_key_event(character as u32, 0),
                false,
                surrounding.clone(),
                false,
            )
            .unwrap();
        assert_eq!(reply.outcome, KeyOutcome::Consumed, "{character}");
        let identity = reply
            .identity
            .clone()
            .expect("Linux Server 在回包上附展示身份");
        assert_eq!(identity.context, "client-test");
        assert!(identity.revision > shown, "按键回包的版本号要严格变大");
        shown = identity.revision;
        last = Some(reply);
    }
    let last = last.unwrap();
    assert!(
        last.frame
            .candidates
            .items
            .iter()
            .any(|candidate| candidate.text == "你好"),
        "nihao 的候选里该有你好：{:?}",
        last.frame.candidates.items
    );
    assert!(!last.frame.preedit.is_empty());

    // 帧没变时轮询沿用版本号；展示回执没有回包，之后的往返照常
    let polled = connection.poll(session).unwrap();
    assert_eq!(polled.revision(), Some(shown));
    connection
        .acknowledge(session, last.identity.clone().unwrap(), Vec::new())
        .unwrap();

    // 组句中 Ctrl+A：Server 放行，冲洗把原文上屏、组句清空
    let flushed = connection
        .key_flushing(
            session,
            to_key_event('a' as u32, CONTROL),
            false,
            None,
            false,
        )
        .unwrap();
    assert_eq!(flushed.outcome, KeyOutcome::Passthrough);
    assert_eq!(flushed.commit.as_deref(), Some("nihao"));
    assert!(flushed.frame.preedit.is_empty());

    // 单击 Shift：按下放行，抬起时切到英文，回包带 english
    let pressed = connection
        .key(
            session,
            to_key_event(keysym::SHIFT_L, 0),
            false,
            None,
            false,
        )
        .unwrap();
    assert_eq!(pressed.outcome, KeyOutcome::Passthrough);
    assert_eq!(pressed.english, None);
    let released = connection
        .key(session, to_key_event(keysym::SHIFT_L, 1), true, None, false)
        .unwrap();
    assert_eq!(released.english, Some(true));

    // 切回中文再组一段，失焦（应用自己画着 preedit）时不上屏
    connection
        .key(
            session,
            to_key_event(keysym::SHIFT_L, 0),
            false,
            None,
            false,
        )
        .unwrap();
    let back = connection
        .key(session, to_key_event(keysym::SHIFT_L, 1), true, None, false)
        .unwrap();
    assert_eq!(back.english, Some(false));
    for character in "ni".chars() {
        connection
            .key(
                session,
                to_key_event(character as u32, 0),
                false,
                None,
                false,
            )
            .unwrap();
    }
    connection.focus(session, false).unwrap();
    let deactivated = connection.deactivate(session, true, true, false).unwrap();
    assert_eq!(deactivated.commit, None);
    connection.close_session(session).unwrap();
    drop(connection);
    drop(server);
    let _ = std::fs::remove_dir_all(&directory);
}
