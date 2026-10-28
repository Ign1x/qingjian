//! 托盘上点的切换：发给活动窗口那个应用的会话，先问现在的模式、不是目标才切；没有输入框时只换模式。

use std::time::Instant;

use qingjian_platform::protocol::linux::{InputMode, ModeCommand};

use super::mode::{chinese, english, ready_with};
use super::support::{Call, FakeServer, Out, Recorder, composing, settled, type_letter};
use crate::session::{Input, ModeTarget, Session};

/// 还没激活过输入框的会话：各应用的模式按 `modes`。
fn idle(modes: &[(Option<&str>, InputMode)]) -> (Session<FakeServer>, Recorder, Instant) {
    let mut server = FakeServer::default();
    for (app, mode) in modes {
        server.modes.insert(app.map(str::to_owned), mode.clone());
    }
    (Session::new(server), Recorder::default(), Instant::now())
}

/// 没有输入框时点托盘：用活动窗口那个应用的会话（下一个输入框进来也是它），问过模式再切，只换模式、不亮提示。
#[test]
fn without_a_field_the_active_app_is_switched() {
    let (mut session, mut out, now) = idle(&[(Some("firefox"), chinese())]);
    session.handle(Input::App(Some("firefox".to_owned())), &mut out, now);
    session.handle(Input::Tray(ModeTarget::English(true)), &mut out, now);
    assert_eq!(
        session.server.calls,
        [
            Call::Select(Some("firefox".to_owned())),
            Call::Poll,
            Call::Indicator(ModeCommand::ToggleEnglish)
        ]
    );
    assert_eq!(
        out.log,
        [Out::Mode(Some(chinese())), Out::Mode(Some(english()))]
    );
}

/// 已经是目标了（托盘上画着的比 Server 的旧）：问过就不切，不会翻反。
#[test]
fn a_switch_already_in_place_is_not_sent() {
    let (mut session, mut out, now) = idle(&[(None, english())]);
    session.handle(Input::Tray(ModeTarget::English(true)), &mut out, now);
    assert_eq!(session.server.calls, [Call::Poll]);
    assert_eq!(out.log, [Out::Mode(Some(english()))]);
}

/// 标点与简繁也按目标：不是才切，切完再点同一个目标什么都不发。
#[test]
fn punctuation_and_traditional_follow_their_targets() {
    let full_width = InputMode {
        full_width: true,
        ..chinese()
    };
    let (mut session, mut out, now) = idle(&[(None, full_width)]);
    session.handle(Input::Tray(ModeTarget::FullWidth(false)), &mut out, now);
    session.handle(Input::Tray(ModeTarget::FullWidth(false)), &mut out, now);
    session.handle(Input::Tray(ModeTarget::Traditional(true)), &mut out, now);
    let indicated: Vec<_> = session
        .server
        .calls
        .iter()
        .filter_map(|call| match call {
            Call::Indicator(command) => Some(*command),
            _ => None,
        })
        .collect();
    assert_eq!(
        indicated,
        [
            ModeCommand::TogglePunctuation,
            ModeCommand::ToggleTraditional
        ]
    );
    assert_eq!(
        out.log.last(),
        Some(&Out::Mode(Some(InputMode {
            traditional: true,
            ..chinese()
        })))
    );
}

/// 输入框里正在组句时从托盘切到英文：与单击 Shift 一样原文上屏、候选窗收起，亮「英」。
#[test]
fn a_switch_while_composing_commits_the_raw_text() {
    let (mut session, mut out, now) = ready_with(&[(None, chinese())]);
    type_letter(&mut session, &mut out, 'n', composing("ni", &["你"]), now);
    settled(&mut session, &mut out, now);
    out.log.clear();
    session.server.calls.clear();
    session.handle(Input::Tray(ModeTarget::English(true)), &mut out, now);
    assert_eq!(
        session.server.calls,
        [Call::Poll, Call::Indicator(ModeCommand::ToggleEnglish)]
    );
    assert_eq!(
        out.log,
        [
            Out::Text("ni".to_owned(), String::new()),
            Out::Mode(Some(english())),
            Out::Hide,
            Out::Hint(english())
        ]
    );
}

/// Server 不在：托盘这一下作废，模式当不知道（托盘换回不带中 / 英的图标）。
#[test]
fn a_switch_without_the_server_forgets_the_mode() {
    let (mut session, mut out, now) = idle(&[(None, chinese())]);
    session.handle(Input::Tray(ModeTarget::English(true)), &mut out, now);
    out.log.clear();
    session.server.down = true;
    session.handle(Input::Tray(ModeTarget::English(false)), &mut out, now);
    assert_eq!(out.log, [Out::Mode(None)]);
}

/// 不知道现在的模式（老 Server 的回包不带）时照发；知道了只在不一样时发。
#[test]
fn targets_turn_into_commands() {
    assert_eq!(
        ModeTarget::English(true).command(None),
        Some(ModeCommand::ToggleEnglish)
    );
    assert_eq!(ModeTarget::English(false).command(Some(&chinese())), None);
    assert_eq!(
        ModeTarget::FullWidth(true).command(Some(&chinese())),
        Some(ModeCommand::TogglePunctuation)
    );
    assert_eq!(
        ModeTarget::Traditional(false).command(Some(&english())),
        None
    );
}

/// 点状态栏后活动窗口变空，切换仍发给刚才的应用。
#[test]
fn losing_the_active_window_keeps_the_recent_session() {
    let (mut session, mut out, now) = idle(&[(Some("foot"), chinese())]);
    session.handle(Input::App(Some("foot".to_owned())), &mut out, now);
    session.handle(Input::Tray(ModeTarget::English(true)), &mut out, now);
    session.handle(Input::App(None), &mut out, now);
    session.server.calls.clear();
    session.handle(Input::Tray(ModeTarget::English(false)), &mut out, now);
    assert_eq!(
        session.server.calls,
        [Call::Poll, Call::Indicator(ModeCommand::ToggleEnglish)]
    );
    assert_eq!(out.log.last(), Some(&Out::Mode(Some(chinese()))));
}

/// 查询模式时焦点已换，不能再发切换并把旧会话的组句上屏到新输入框。
#[test]
fn a_focus_change_during_poll_cancels_the_switch() {
    let (mut session, mut out, now) = ready_with(&[(None, chinese())]);
    out.switch_on_sync = true;
    session.handle(Input::Tray(ModeTarget::English(true)), &mut out, now);
    assert_eq!(session.server.calls, [Call::Poll]);
    assert!(out.log.is_empty());
}
