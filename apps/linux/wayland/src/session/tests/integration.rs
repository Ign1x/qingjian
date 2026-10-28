//! 模式提示、托盘与候选鼠标操作共用会话时的回归。

use std::time::Instant;

use qingjian_platform::protocol::KeyOutcome;
use qingjian_platform::protocol::linux::client::Reply;
use qingjian_platform::protocol::linux::{InputMode, MenuItem, ModeCommand};

use super::mode::{chinese, english, ready_with};
use super::support::{Call, FakeServer, Out, Recorder, answer, composing, settled, type_letter};
use crate::session::{Click, HINT, Input, ModeTarget, POLL_INTERVAL, Session, Spot};

fn click(spot: Spot, right: bool) -> Input {
    Input::Click(Click {
        spot: Some(spot),
        right,
        position: (0.0, 0.0),
        epoch: 0,
    })
}

fn opened() -> (Session<FakeServer>, Recorder, Instant) {
    let (mut session, mut out, now) = ready_with(&[(None, chinese())]);
    type_letter(
        &mut session,
        &mut out,
        'n',
        composing("n", &["你", "呢"]),
        now,
    );
    settled(&mut session, &mut out, now);
    session.server.menu = vec![MenuItem {
        id: 5,
        label: "删除".to_owned(),
    }];
    session.handle(click(Spot::Candidate(1), true), &mut out, now);
    assert!(session.menu.is_some());
    session.server.calls.clear();
    out.log.clear();
    (session, out, now)
}

/// 空帧换成模式提示时，旧候选菜单和点击身份也必须一起清掉。
#[test]
fn an_empty_frame_clears_pointer_state_before_the_hint() {
    let (mut session, mut out, now) = opened();
    session.apply(
        Reply {
            mode: Some(english()),
            english: Some(true),
            ..Reply::default()
        },
        0,
        &mut out,
        now,
    );
    assert!(session.menu.is_none());
    assert!(session.identity.is_none());
    assert_eq!(session.hint_until, Some(now + HINT));
    assert!(out.log.contains(&Out::Hide));
    assert!(out.log.contains(&Out::Hint(english())));
    session.handle(click(Spot::MenuItem(0), false), &mut out, now);
    assert!(session.server.calls.is_empty());
}

/// 模式与候选内容是两种状态；轮询的版本号不变时，模式变化也要重画。
#[test]
fn a_mode_change_redraws_even_at_the_same_revision() {
    let (mut session, mut out, now) = ready_with(&[(None, chinese())]);
    type_letter(&mut session, &mut out, 'n', composing("n", &["你"]), now);
    let identity = session.identity.clone();
    let mode = InputMode {
        full_width: true,
        ..chinese()
    };
    session.server.modes.insert(None, mode.clone());
    out.log.clear();
    session.poll(&mut out, now);
    assert_eq!(session.identity, identity, "轮询沿用版本号");
    assert_eq!(
        out.log,
        [Out::Mode(Some(mode)), Out::Show(vec!["你".to_owned()])]
    );
}

/// 托盘切标点会重画同一帧，旧菜单先收起，之后点选使用新的展示身份。
#[test]
fn a_tray_change_closes_the_menu_and_refreshes_clicks() {
    let (mut session, mut out, now) = opened();
    session.handle(Input::Tray(ModeTarget::FullWidth(true)), &mut out, now);
    assert_eq!(out.log.first(), Some(&Out::CloseMenu));
    assert!(session.menu.is_none());
    assert_eq!(session.poll_at, Some(now + POLL_INTERVAL));
    assert_eq!(
        &session.server.calls[..2],
        [Call::Poll, Call::Indicator(ModeCommand::TogglePunctuation)]
    );
    let identity = session.identity.clone().unwrap();
    session
        .server
        .clicks
        .push_back(answer(KeyOutcome::Consumed, Some("呢"), Default::default()));
    session.handle(click(Spot::Candidate(1), false), &mut out, now);
    assert!(
        session
            .server
            .calls
            .contains(&Call::Candidate(1, identity.revision))
    );
    assert!(out.log.contains(&Out::Text("呢".to_owned(), String::new())));
}

/// 即使托盘目标已经满足，查询也可能换帧，因此不能留下查询前的候选菜单。
#[test]
fn a_satisfied_tray_target_still_closes_the_candidate_menu() {
    let (mut session, mut out, now) = opened();
    session.handle(Input::Tray(ModeTarget::English(false)), &mut out, now);
    assert_eq!(session.server.calls, [Call::Poll]);
    assert_eq!(out.log, [Out::CloseMenu]);
    assert!(session.menu.is_none());
    assert_eq!(session.poll_at, Some(now + POLL_INTERVAL));
}

/// 没有输入框时查询也会等 Server；期间激活了新输入框必须先同步，不能继续切旧会话。
#[test]
fn an_idle_tray_poll_checks_for_a_new_input_target() {
    let mut server = FakeServer::default();
    server.modes.insert(None, chinese());
    let mut session = Session::new(server);
    let mut out = Recorder {
        switch_on_sync: true,
        ..Recorder::default()
    };
    session.handle(
        Input::Tray(ModeTarget::English(true)),
        &mut out,
        Instant::now(),
    );
    assert_eq!(session.server.calls, [Call::Poll]);
    assert!(out.log.is_empty());
}
