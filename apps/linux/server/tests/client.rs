//! 用两个 Rust 前端共用的客户端（`qingjian_platform::protocol::linux::client`）对着真 Server 走一遍：
//! 握手、按键往返、轮询的版本号、展示回执、快捷键冲洗、Shift 单击切换、失焦不上屏，
//! 以及 v4 的右键菜单、托盘切换与回包上的模式。
#![cfg(target_os = "linux")]
#[path = "support/server.rs"]
mod server;

use std::path::{Path, PathBuf};

use qingjian_platform::Config;
use qingjian_platform::candidate_menu::CandidateAction;
use qingjian_platform::protocol::linux::client::{Connection, Reply};
use qingjian_platform::protocol::linux::key::{keysym, to_key_event};
use qingjian_platform::protocol::linux::{AppSurrounding, Capabilities, InputMode, ModeCommand};
use qingjian_platform::protocol::{KeyOutcome, SessionId};
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

/// 起一个 Server（配置文件写 `config`），开好一个会话、报过能力与焦点；返回 Server、临时目录、配置文件路径与连接。
fn start(name: &str, config: &str) -> (Server, PathBuf, PathBuf, Connection, SessionId) {
    let directory =
        std::env::temp_dir().join(format!("qingjian-client-{name}-{}", std::process::id()));
    let path = directory.join("config/qingjian/config.toml");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, config).unwrap();
    let mut server = Server::start(directory.clone());
    drop(server.connect());
    let mut connection = Connection::connect_at(&directory.join("server.sock")).unwrap();
    let session = connection.open_session(name).unwrap();
    connection
        .capabilities(session, Capabilities::default())
        .unwrap();
    connection.focus(session, true).unwrap();
    (server, directory, path, connection, session)
}

/// 逐个字母报上去，返回最后一键的回包。
fn type_text(connection: &mut Connection, session: SessionId, text: &str) -> Reply {
    let mut last = Reply::default();
    for character in text.chars() {
        last = connection
            .key(
                session,
                to_key_event(character as u32, 0),
                false,
                None,
                false,
            )
            .unwrap();
    }
    last
}

fn general(path: &Path) -> qingjian_platform::GeneralConfig {
    Config::load(path).unwrap().general
}

#[test]
fn shared_client_toggles_modes_against_the_real_server() {
    let (server, directory, path, mut connection, session) =
        start("mode", "[shortcut]\nswitch_mode = \"shift\"\n");
    let typed = type_text(&mut connection, session, "nihao");
    assert_eq!(
        typed.mode,
        Some(InputMode {
            full_width: true,
            ..InputMode::default()
        }),
        "每个回包都附上当前模式"
    );

    // 托盘：切到英文，组句原样上屏，回包带 english 与新模式
    let english = connection
        .indicator(session, ModeCommand::ToggleEnglish)
        .unwrap();
    assert_eq!(english.english, Some(true));
    assert_eq!(english.commit.as_deref(), Some("nihao"));
    assert!(english.mode.as_ref().is_some_and(|mode| mode.english));

    // 英文模式那一份全角标点单独记，写回配置文件
    let punctuation = connection
        .indicator(session, ModeCommand::TogglePunctuation)
        .unwrap();
    assert!(
        punctuation
            .mode
            .as_ref()
            .is_some_and(|mode| mode.full_width)
    );
    assert!(general(&path).english_full_width_punctuation);

    let chinese = connection
        .indicator(session, ModeCommand::ToggleEnglish)
        .unwrap();
    assert_eq!(chinese.english, Some(false));
    let traditional = connection
        .indicator(session, ModeCommand::ToggleTraditional)
        .unwrap();
    assert!(traditional.mode.is_some_and(|mode| mode.traditional));
    assert_eq!(general(&path).traditional_variant, "taiwan");

    connection.close_session(session).unwrap();
    drop(connection);
    drop(server);
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn shared_client_drives_the_candidate_menu_against_the_real_server() {
    let (server, directory, _, mut connection, session) = start("menu", "");
    let typed = type_text(&mut connection, session, "nihao");
    let identity = typed.identity.clone().unwrap();
    let slot = typed
        .frame
        .candidates
        .items
        .iter()
        .position(|candidate| candidate.text == "你好")
        .expect("nihao 的候选里该有你好");

    // 右键：问这一格能做什么，帧不变；点「降权」做完带提示、换一帧
    let menu = connection.menu(session, identity.clone(), slot).unwrap();
    let labels: Vec<_> = menu.menu.iter().map(|item| item.label.as_str()).collect();
    assert!(
        labels.contains(&"降权") && labels.contains(&"删除"),
        "{labels:?}"
    );
    assert!(menu.identity.is_none());
    let picked = connection
        .menu_pick(
            session,
            identity.clone(),
            slot,
            CandidateAction::Demote.id(),
        )
        .unwrap();
    assert_eq!(picked.outcome, KeyOutcome::Consumed);
    let notice = picked.frame.notice.clone().expect("做完带提示");
    assert!(notice.contains("你好"), "{notice}");
    assert!(picked.revision() > Some(identity.revision));
    let stale = connection.menu(session, identity, slot).unwrap();
    assert!(stale.menu.is_empty(), "过期的帧问不到菜单");

    connection.close_session(session).unwrap();
    drop(connection);
    drop(server);
    let _ = std::fs::remove_dir_all(&directory);
}
