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

    let session = connection.open_session("client-test", None).unwrap();
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
    let session = connection.open_session(name, None).unwrap();
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
    let labels: Vec<_> = menu.iter().map(|item| item.label.as_str()).collect();
    assert!(
        labels.contains(&"降权") && labels.contains(&"删除"),
        "{labels:?}"
    );
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
    assert!(stale.is_empty(), "过期的帧问不到菜单");

    connection.close_session(session).unwrap();
    drop(connection);
    drop(server);
    let _ = std::fs::remove_dir_all(&directory);
}

/// 同一条连接上两个应用各一个会话（Wayland 前端按应用分会话就是这样），在 A 里单击 Shift 切到英文；返回这两个会话。
/// `start` 开的那个会话没报应用名，先让出焦点。
fn two_apps_after_switching_the_first(
    connection: &mut Connection,
    opened: SessionId,
) -> (SessionId, SessionId) {
    connection.focus(opened, false).unwrap();
    let first = connection.open_session("per-app-a", Some("foot")).unwrap();
    let second = connection
        .open_session("per-app-b", Some("org.gnome.TextEditor"))
        .unwrap();
    for session in [first, second] {
        connection
            .capabilities(session, Capabilities::default())
            .unwrap();
    }
    connection.focus(first, true).unwrap();
    connection
        .key(first, to_key_event(keysym::SHIFT_L, 0), false, None, false)
        .unwrap();
    let toggled = connection
        .key(first, to_key_event(keysym::SHIFT_L, 1), true, None, false)
        .unwrap();
    assert_eq!(toggled.english, Some(true));
    connection.focus(first, false).unwrap();
    (first, second)
}

/// 缺省所有应用共用一份中英模式：A 里切到英文，B 也是英文。
#[test]
fn modes_are_shared_by_all_apps_by_default() {
    let (server, directory, _path, mut connection, opened) =
        start("shared-mode", "[shortcut]\nswitch_mode = \"shift\"\n");
    let (_, second) = two_apps_after_switching_the_first(&mut connection, opened);
    let focused = connection.focus(second, true).unwrap();
    assert!(
        focused.mode.as_ref().is_some_and(|mode| mode.english),
        "B 跟着是英文"
    );
    drop(connection);
    drop(server);
    let _ = std::fs::remove_dir_all(&directory);
}

/// 打开「按应用记住中英模式」：A 里切到英文，B 仍是中文，切回 A 时它还记得英文——`OpenSession.app` 原样带到 Server，
/// Server 按它记；关掉 A 的会话再开（Wayland 前端换掉最久没用的会话就是这样），按应用名记着的不丢。
#[test]
fn modes_are_kept_per_app_on_one_connection() {
    let (server, directory, _path, mut connection, opened) = start(
        "per-app",
        "[general]\nremember_mode_per_app = true\n[shortcut]\nswitch_mode = \"shift\"\n",
    );
    let (first, second) = two_apps_after_switching_the_first(&mut connection, opened);
    connection.focus(second, true).unwrap();
    let typed = type_text(&mut connection, second, "ni");
    assert_eq!(typed.outcome, KeyOutcome::Consumed, "B 没被 A 的切换带走");
    assert!(typed.mode.as_ref().is_some_and(|mode| !mode.english));
    connection.reset(second).unwrap();

    connection.focus(second, false).unwrap();
    let back = connection.focus(first, true).unwrap();
    assert!(
        back.mode.as_ref().is_some_and(|mode| mode.english),
        "A 还记得英文"
    );
    connection.focus(first, false).unwrap();
    connection.close_session(first).unwrap();
    let reopened = connection.open_session("per-app-c", Some("foot")).unwrap();
    connection
        .capabilities(reopened, Capabilities::default())
        .unwrap();
    let again = connection.focus(reopened, true).unwrap();
    assert!(
        again.mode.as_ref().is_some_and(|mode| mode.english),
        "同一个应用重开会话还是英文"
    );
    drop(connection);
    drop(server);
    let _ = std::fs::remove_dir_all(&directory);
}
