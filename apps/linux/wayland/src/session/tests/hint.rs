//! 中 / 英提示：切换的那一下在候选窗的位置亮一会儿，到点、来了候选或失焦就换下去。

use std::time::Instant;

use qingjian_platform::protocol::linux::key::keysym;
use qingjian_platform::protocol::{Frame, KeyOutcome};

use super::super::{HINT, Input, Session};
use super::mode::{chinese, english, ready_with};
use super::support::{Answer, FakeServer, Out, Recorder, answer, composing, key, type_letter};

/// 单击 Shift 切到英文：抬键的回包带着 `english` 与新模式。
fn switch_to_english(session: &mut Session<FakeServer>, out: &mut Recorder, now: Instant) {
    session.handle(Input::Key(key(42, keysym::SHIFT_L, true)), out, now);
    session.server.modes.insert(None, english());
    session.server.releases.push_back(Answer {
        english: Some(true),
        ..answer(KeyOutcome::Consumed, None, Frame::default())
    });
    session.handle(Input::Key(key(42, keysym::SHIFT_L, false)), out, now);
}

/// 切到英文：交出新模式，在候选窗的位置亮「英」，到点收起。
#[test]
fn a_switch_lights_the_hint_until_it_times_out() {
    let (mut session, mut out, now) = ready_with(&[(None, chinese())]);
    switch_to_english(&mut session, &mut out, now);
    assert_eq!(
        out.log,
        [
            Out::Forward(42, true),
            Out::Mode(Some(english())),
            Out::Hint(english()),
            Out::Forward(42, false)
        ]
    );
    assert_eq!(session.deadline(), Some(now + HINT));
    out.log.clear();

    session.tick(&mut out, now + HINT / 2);
    assert!(out.log.is_empty(), "还没到点：{:?}", out.log);
    session.tick(&mut out, now + HINT);
    assert_eq!(out.log, [Out::Hide]);
    assert_eq!(session.deadline(), None);
}

/// 提示亮着时别的键的回包是空帧（Ctrl + Alt + Space 切换时抬起另几个键也是）：不收提示，照样到点才收。
#[test]
fn empty_frames_leave_the_hint_alone() {
    let (mut session, mut out, now) = ready_with(&[(None, chinese())]);
    switch_to_english(&mut session, &mut out, now);
    out.log.clear();
    session.handle(Input::Key(key(29, keysym::CONTROL_L, true)), &mut out, now);
    session.handle(Input::Key(key(29, keysym::CONTROL_L, false)), &mut out, now);
    assert_eq!(out.log, [Out::Forward(29, true), Out::Forward(29, false)]);
    session.tick(&mut out, now + HINT);
    assert_eq!(out.log.last(), Some(&Out::Hide));
}

/// 提示亮着时开始打字：候选窗换下提示，到点不收候选。
#[test]
fn candidates_take_over_the_hint() {
    let (mut session, mut out, now) = ready_with(&[(None, chinese())]);
    switch_to_english(&mut session, &mut out, now);
    out.log.clear();
    type_letter(&mut session, &mut out, 'h', composing("h", &["hello"]), now);
    assert!(
        out.log.contains(&Out::Show(vec!["hello".to_owned()])),
        "{:?}",
        out.log
    );
    assert!(!out.log.contains(&Out::Hide));
    out.log.clear();
    session.tick(&mut out, now + HINT);
    assert!(!out.log.contains(&Out::Hide), "{:?}", out.log);
}

/// 提示亮着时输入框失焦：跟着候选窗一起收，之后到点也不再收一次。
#[test]
fn the_hint_leaves_with_the_field() {
    let (mut session, mut out, now) = ready_with(&[(None, chinese())]);
    switch_to_english(&mut session, &mut out, now);
    out.log.clear();
    session.handle(Input::Deactivate, &mut out, now);
    assert_eq!(out.log, [Out::Hide]);
    out.log.clear();
    session.tick(&mut out, now + HINT);
    assert!(!out.log.contains(&Out::Hide), "{:?}", out.log);
}

/// 切换的回包还带着候选（少见：Server 切完仍在组句）：不拿提示盖住候选。
#[test]
fn no_hint_over_candidates() {
    let (mut session, mut out, now) = ready_with(&[(None, chinese())]);
    session.server.modes.insert(None, english());
    session.server.presses.push_back(Answer {
        english: Some(true),
        ..answer(KeyOutcome::Consumed, None, composing("h", &["hello"]))
    });
    session.handle(Input::Key(key(57, ' ' as u32, true)), &mut out, now);
    assert!(
        !out.log.iter().any(|entry| matches!(entry, Out::Hint(_))),
        "{:?}",
        out.log
    );
    assert!(session.hint_until.is_none());
}
