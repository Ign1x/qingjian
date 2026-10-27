//! 转发、抬起跟着按下走、单击 Shift、重复、键位表。

use std::time::Duration;

use qingjian_platform::protocol::linux::key::keysym;
use qingjian_platform::protocol::{Frame, KeyOutcome};

use super::super::{Input, Modifiers, Session};
use super::support::{
    Answer, FakeServer, Out, Recorder, answer, composing, key, ready, settled, type_letter,
};

/// 没在组句时放行的键：按下与抬起都转给应用，不往应用里放任何文字。
#[test]
fn passthrough_keys_reach_the_app_with_their_release() {
    let (mut session, mut out, now) = ready();
    session.handle(Input::Key(key(30, 'a' as u32, true)), &mut out, now);
    session.handle(Input::Key(key(30, 'a' as u32, false)), &mut out, now);
    assert_eq!(out.log, [Out::Forward(30, true), Out::Forward(30, false)]);
}

/// 单击 Shift：按下放行（转给应用），抬起时 Server 切到英文并交回原文——先上屏，再转抬起，
/// 哪怕 Server 说抬起吃掉了：应用收到过按下，不转抬起的话 Shift 在应用里一直按着。
#[test]
fn shift_tap_switches_and_the_release_still_reaches_the_app() {
    let (mut session, mut out, now) = ready();
    type_letter(&mut session, &mut out, 'n', composing("ni", &["你"]), now);
    out.log.clear();
    session.handle(Input::Key(key(42, keysym::SHIFT_L, true)), &mut out, now);
    session.server.releases.push_back(Answer {
        english: Some(true),
        ..answer(KeyOutcome::Consumed, Some("ni"), Frame::default())
    });
    session.handle(Input::Key(key(42, keysym::SHIFT_L, false)), &mut out, now);
    assert_eq!(
        out.log,
        [Out::Forward(42, true), Out::Hide, Out::Forward(42, false)],
        "上一次更新应用还没落定：原文先攒着，抬起照转"
    );
    settled(&mut session, &mut out, now);
    assert_eq!(
        out.log.last(),
        Some(&Out::Text("ni".to_owned(), String::new()))
    );
}

/// grab 之前就按着的键：抬起转给应用，不报 Server。
#[test]
fn releases_of_unknown_keys_are_forwarded_without_asking() {
    let (mut session, mut out, now) = ready();
    session.handle(Input::Key(key(29, keysym::CONTROL_L, false)), &mut out, now);
    assert_eq!(out.log, [Out::Forward(29, false)]);
    assert!(session.server.calls.is_empty());
}

/// 组句中按住退格：吃掉的键自己重复；删空后 Server 放行，就转一次按下，重复交给应用，抬起也转。
#[test]
fn held_backspace_repeats_until_the_server_lets_go() {
    let (mut session, mut out, now) = ready();
    type_letter(&mut session, &mut out, 'n', composing("ni", &["你"]), now);
    session
        .server
        .presses
        .push_back(answer(KeyOutcome::Consumed, None, composing("n", &["你"])));
    session.handle(Input::Key(key(14, keysym::BACKSPACE, true)), &mut out, now);
    settled(&mut session, &mut out, now);
    out.log.clear();

    let first = now + Duration::from_millis(600);
    assert_eq!(session.deadline(), Some(now + Duration::from_millis(80)));
    session
        .server
        .presses
        .push_back(answer(KeyOutcome::Consumed, None, Frame::default()));
    session.tick(&mut out, first);
    assert_eq!(
        out.log,
        [Out::Text(String::new(), String::new()), Out::Hide],
        "第一次重复：删到空"
    );
    out.log.clear();

    session
        .server
        .presses
        .push_back(answer(KeyOutcome::Passthrough, None, Frame::default()));
    session.tick(&mut out, first + Duration::from_millis(40));
    assert_eq!(out.log, [Out::Forward(14, true)], "组句空了：按下交给应用");
    assert_eq!(session.deadline(), None, "重复交给应用之后自己不再重复");
    out.log.clear();

    session.handle(Input::Key(key(14, keysym::BACKSPACE, false)), &mut out, now);
    assert_eq!(out.log, [Out::Forward(14, false)]);
}

/// 虚拟键盘收到键位表之前什么都不能发（协议错误会断开整条连接）；同一份键位表不重发。
#[test]
fn nothing_reaches_the_virtual_keyboard_before_a_keymap() {
    let mut session = Session::new(FakeServer::default());
    let mut out = Recorder::default();
    let now = std::time::Instant::now();
    session.handle(Input::Activate(Default::default()), &mut out, now);
    out.log.clear();
    session.handle(Input::Modifiers(Modifiers::default()), &mut out, now);
    session.handle(Input::Key(key(30, 'a' as u32, true)), &mut out, now);
    assert!(out.log.is_empty(), "{:?}", out.log);

    session.handle(Input::Keymap("a".to_owned()), &mut out, now);
    session.handle(Input::Keymap("a".to_owned()), &mut out, now);
    session.handle(
        Input::Modifiers(Modifiers {
            depressed: 1,
            ..Modifiers::default()
        }),
        &mut out,
        now,
    );
    assert_eq!(out.log, [Out::Keymap, Out::Modifiers(1)]);
}
