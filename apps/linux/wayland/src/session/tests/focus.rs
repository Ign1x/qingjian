//! 输入框进出、密码框、Server 断开与重连。

use std::time::Duration;

use qingjian_platform::protocol::linux::key::keysym;

use super::super::{Field, Input, Session, Surrounding};
use super::support::{Call, FakeServer, Out, Recorder, composing, key, ready, type_letter};

/// 密码框：报能力，不报焦点、不接管键盘。
#[test]
fn a_password_field_is_left_alone() {
    let mut session = Session::new(FakeServer::default());
    let mut out = Recorder::default();
    let now = std::time::Instant::now();
    let field = Field {
        password: true,
        ..Field::default()
    };
    session.handle(Input::Activate(field), &mut out, now);
    assert!(out.log.is_empty(), "{:?}", out.log);
    assert!(matches!(session.server.calls[..], [Call::Capabilities(caps)] if caps.password));
}

/// 失焦时还按着的键：等它抬起（经虚拟键盘转出去）再交还键盘，虚拟键盘上才不会卡住。
#[test]
fn the_grab_is_kept_until_held_keys_are_released() {
    let (mut session, mut out, now) = ready();
    type_letter(&mut session, &mut out, 'n', composing("ni", &["你"]), now);
    session.handle(Input::Key(key(42, keysym::SHIFT_L, true)), &mut out, now);
    out.log.clear();

    session.handle(Input::Deactivate, &mut out, now);
    session.tick(&mut out, now);
    assert_eq!(out.log, [Out::Hide], "还按着 Shift：不交还键盘");
    assert!(
        session
            .server
            .calls
            .ends_with(&[Call::Focus(false), Call::Deactivate])
    );
    out.log.clear();

    let calls = session.server.calls.len();
    session.handle(Input::Key(key(42, keysym::SHIFT_L, false)), &mut out, now);
    session.tick(&mut out, now);
    assert_eq!(out.log, [Out::Forward(42, false), Out::Grab(false)]);
    assert_eq!(session.server.calls.len(), calls, "失焦后的键不报 Server");
}

/// 激活时 Server 不在：先不接管键盘，退避到点后再连，连上就接管。
#[test]
fn an_unreachable_server_is_retried_later() {
    let mut server = FakeServer::default();
    server.down = true;
    let mut session = Session::new(server);
    let mut out = Recorder::default();
    let now = std::time::Instant::now();
    session.handle(Input::Activate(Field::default()), &mut out, now);
    assert!(out.log.is_empty());
    let retry = session.deadline().expect("排了重连");
    assert!(retry >= now + Duration::from_secs(1));

    session.server.down = false;
    session.tick(&mut out, retry);
    assert_eq!(out.log, [Out::Grab(true)]);
}

/// 组句中 Server 没了：拼音清掉、候选窗收起，键原样交给应用；键都松开后交还键盘。
#[test]
fn losing_the_server_mid_composition_passes_keys_through() {
    let (mut session, mut out, now) = ready();
    type_letter(&mut session, &mut out, 'n', composing("ni", &["你"]), now);
    out.log.clear();
    session.server.down = true;
    session.handle(Input::Key(key(35, 'h' as u32, true)), &mut out, now);
    session.handle(Input::Key(key(35, 'h' as u32, false)), &mut out, now);
    session.tick(&mut out, now);
    assert_eq!(
        out.log,
        [
            Out::Text(String::new(), String::new()),
            Out::Hide,
            Out::Forward(35, true),
            Out::Forward(35, false),
            Out::Grab(false),
        ]
    );
}

/// 组句中应用自己挪了光标（前后文变了、原因是 other）：丢掉组句。输入法上屏引起的变化不算。
#[test]
fn an_external_edit_while_composing_resets() {
    let (mut session, mut out, now) = ready();
    type_letter(&mut session, &mut out, 'n', composing("ni", &["你"]), now);
    out.log.clear();
    let moved = |text: &str, external_change| Field {
        surrounding: Some(Surrounding {
            text: text.to_owned(),
            cursor: 0,
            anchor: 0,
        }),
        external_change,
        ..Field::default()
    };
    session.handle(Input::Update(moved("甲", false)), &mut out, now);
    assert!(out.log.is_empty());

    session.handle(Input::Update(moved("乙", true)), &mut out, now);
    assert_eq!(session.server.calls.last(), Some(&Call::Reset));
    assert_eq!(
        out.log,
        [Out::Text(String::new(), String::new()), Out::Hide]
    );
}

/// 又激活一次（同一个应用里换了输入框）：先当失活处理，再接着接管。
#[test]
fn reactivation_drops_the_old_composition() {
    let (mut session, mut out, now) = ready();
    type_letter(&mut session, &mut out, 'n', composing("ni", &["你"]), now);
    out.log.clear();
    session.handle(Input::Deactivate, &mut out, now);
    session.handle(Input::Activate(Field::default()), &mut out, now);
    session.tick(&mut out, now);
    assert_eq!(out.log, [Out::Hide], "键盘一直在手里，不来回放");
    assert!(session.server.calls.ends_with(&[
        Call::Focus(false),
        Call::Deactivate,
        Call::Capabilities(Field::default().capabilities()),
        Call::Focus(true),
    ]));
}
