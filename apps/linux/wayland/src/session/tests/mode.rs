//! 输入模式：回包上的模式交给显示面（主题的 `mode.*`），换了应用跟着换，与 Server 断了就当不知道。

use std::time::Instant;

use qingjian_platform::protocol::KeyOutcome;
use qingjian_platform::protocol::linux::InputMode;

use super::super::{Field, Input, Session};
use super::support::{FakeServer, Out, Recorder, answer, composing, key, type_letter};

pub fn chinese() -> InputMode {
    InputMode::default()
}

pub fn english() -> InputMode {
    InputMode {
        english: true,
        ..InputMode::default()
    }
}

/// 一个会话，各应用的模式按 `modes`（`None` 是不知道哪个应用的那一个），已经拿到焦点、收到键位表；两边的记录都清空。
pub fn ready_with(modes: &[(Option<&str>, InputMode)]) -> (Session<FakeServer>, Recorder, Instant) {
    let mut server = FakeServer::default();
    for (app, mode) in modes {
        server.modes.insert(app.map(str::to_owned), mode.clone());
    }
    let mut session = Session::new(server);
    let mut out = Recorder::default();
    let now = Instant::now();
    session.handle(Input::Activate(Field::default()), &mut out, now);
    session.handle(Input::Keymap("xkb_keymap {};".to_owned()), &mut out, now);
    session.server.calls.clear();
    out.log.clear();
    (session, out, now)
}

/// 激活时焦点的回包带着模式，交给显示面；之后的回包模式没变不再交，变了再交。
#[test]
fn the_mode_reaches_the_display_when_it_changes() {
    let mut server = FakeServer::default();
    server.modes.insert(None, chinese());
    let mut session = Session::new(server);
    let mut out = Recorder::default();
    let now = Instant::now();
    session.handle(Input::Activate(Field::default()), &mut out, now);
    assert_eq!(out.log, [Out::Mode(Some(chinese())), Out::Grab(true)]);
    session.handle(Input::Keymap("xkb_keymap {};".to_owned()), &mut out, now);
    out.log.clear();

    session.handle(Input::Key(key(30, 'a' as u32, true)), &mut out, now);
    assert_eq!(out.log, [Out::Forward(30, true)], "模式没变不交");

    let full_width = InputMode {
        full_width: true,
        ..chinese()
    };
    session.server.modes.insert(None, full_width.clone());
    session.handle(Input::Key(key(30, 'a' as u32, false)), &mut out, now);
    assert_eq!(
        out.log,
        [
            Out::Forward(30, true),
            Out::Mode(Some(full_width)),
            Out::Forward(30, false)
        ]
    );
}

/// 候选窗开着时模式变了（托盘切了全角标点，帧没变）：重画一次，主题里的 `mode.*` 才跟得上。
#[test]
fn a_new_mode_redraws_the_candidates() {
    let (mut session, mut out, now) = ready_with(&[(None, chinese())]);
    type_letter(&mut session, &mut out, 'n', composing("n", &["你"]), now);
    out.log.clear();
    session.server.modes.insert(
        None,
        InputMode {
            full_width: true,
            ..chinese()
        },
    );
    session
        .server
        .presses
        .push_back(answer(KeyOutcome::Consumed, None, composing("n", &["你"])));
    session.handle(Input::Key(key(50, 'm' as u32, true)), &mut out, now);
    assert!(
        matches!(&out.log[..], [Out::Mode(Some(_)), Out::Show(texts)] if texts == &["你"]),
        "{:?}",
        out.log
    );
}

/// 换到另一个应用的输入框：用它的会话，模式也换成它的。
#[test]
fn each_app_brings_its_own_mode() {
    let (mut session, mut out, now) = ready_with(&[(None, chinese()), (Some("foot"), english())]);
    session.handle(Input::Deactivate, &mut out, now);
    session.handle(Input::App(Some("foot".to_owned())), &mut out, now);
    out.log.clear();
    session.handle(Input::Activate(Field::default()), &mut out, now);
    assert_eq!(out.log, [Out::Mode(Some(english()))]);
}

/// 与 Server 的往返断了：Server 那边的会话连同模式都没了，显示面当不知道。
#[test]
fn losing_the_server_forgets_the_mode() {
    let (mut session, mut out, now) = ready_with(&[(None, chinese())]);
    session.server.down = true;
    session.handle(Input::Key(key(30, 'a' as u32, true)), &mut out, now);
    assert_eq!(
        out.log,
        [Out::Mode(None), Out::Forward(30, true)],
        "{:?}",
        out.log
    );
}
