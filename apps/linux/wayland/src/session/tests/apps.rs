//! 一个应用一个会话：输入框激活时用活动窗口那个应用的会话；激活期间活动窗口换了应用，等一会儿、不在组句时跟过去。

use std::time::Instant;

use qingjian_platform::protocol::{Frame, KeyOutcome};

use super::super::{FOLLOW, Field, Input, Server, Session};
use super::support::{Call, FakeServer, Out, Recorder, answer, composing, key, ready, type_letter};

fn app(name: &str) -> Input {
    Input::App(Some(name.to_owned()))
}

fn select(name: &str) -> Call {
    Call::Select(Some(name.to_owned()))
}

/// 换到某个应用的会话时 Server 收到的：旧会话失焦、失活，选新应用，报能力与焦点。
fn switched(name: &str) -> [Call; 5] {
    [
        Call::Focus(false),
        Call::Deactivate,
        select(name),
        Call::Capabilities(Field::default().capabilities()),
        Call::Focus(true),
    ]
}

/// 输入框激活时用活动窗口那个应用的会话；没有输入框激活着时活动窗口换了只记下，下一个输入框激活时换过去。
#[test]
fn each_field_uses_the_session_of_its_app() {
    let mut session = Session::new(FakeServer::default());
    let mut out = Recorder::default();
    let now = Instant::now();
    session.handle(app("firefox"), &mut out, now);
    assert!(session.server.calls.is_empty(), "没有输入框激活着：只记下");
    session.handle(Input::Activate(Field::default()), &mut out, now);
    session.handle(Input::Deactivate, &mut out, now);
    session.handle(app("foot"), &mut out, now);
    session.handle(Input::Activate(Field::default()), &mut out, now);

    let capabilities = Call::Capabilities(Field::default().capabilities());
    assert_eq!(
        session.server.calls,
        [
            select("firefox"),
            capabilities.clone(),
            Call::Focus(true),
            Call::Focus(false),
            Call::Deactivate,
            select("foot"),
            capabilities,
            Call::Focus(true),
        ]
    );
    assert_eq!(session.deadline(), None, "没有要跟过去的");
}

/// 合成器先报了激活、后报活动窗口（或应用改了 app_id）：等一会儿再跟过去，键盘、行内拼音与候选窗都不动。
#[test]
fn a_late_app_is_followed_after_a_moment() {
    let (mut session, mut out, now) = ready();
    session.handle(app("firefox"), &mut out, now);
    assert!(session.server.calls.is_empty(), "先不换");
    assert_eq!(session.deadline(), Some(now + FOLLOW));

    session.tick(&mut out, now + FOLLOW);
    assert_eq!(session.server.calls, switched("firefox"));
    assert!(out.log.is_empty(), "{:?}", out.log);
    assert_eq!(session.deadline(), None);
}

/// 组句中活动窗口换了应用：换了这次组句就丢了，所以等它上屏之后再换。
#[test]
fn following_waits_for_the_composition() {
    let (mut session, mut out, now) = ready();
    type_letter(&mut session, &mut out, 'n', composing("ni", &["你"]), now);
    session.handle(app("firefox"), &mut out, now);
    let later = now + FOLLOW;
    session.tick(&mut out, later);
    assert!(
        !session.server.calls.contains(&select("firefox")),
        "组句中不换"
    );
    assert_eq!(session.server.app(), None);

    session
        .server
        .presses
        .push_back(answer(KeyOutcome::Consumed, Some("你"), Frame::default()));
    session.handle(Input::Key(key(57, ' ' as u32, true)), &mut out, later);
    session.handle(Input::Key(key(57, ' ' as u32, false)), &mut out, later);
    assert!(
        out.log.contains(&Out::Text("你".to_owned(), String::new())),
        "上屏照常：{:?}",
        out.log
    );
    session.server.calls.clear();
    session.tick(&mut out, later + FOLLOW);
    assert_eq!(session.server.calls, switched("firefox"));
}

/// 切换窗口时活动窗口先变（中间还有一下没有活动窗口），输入框随后失活又激活：直接用上新应用，等着的那一下不再白换。
#[test]
fn a_focus_change_supersedes_following() {
    let mut session = Session::new(FakeServer::default());
    let mut out = Recorder::default();
    let now = Instant::now();
    session.handle(app("firefox"), &mut out, now);
    session.handle(Input::Activate(Field::default()), &mut out, now);
    session.server.calls.clear();

    session.handle(Input::App(None), &mut out, now);
    session.handle(app("foot"), &mut out, now);
    session.handle(Input::Deactivate, &mut out, now);
    session.handle(Input::Activate(Field::default()), &mut out, now);
    session.tick(&mut out, now + FOLLOW);
    assert_eq!(session.server.calls, switched("foot"));
}

/// 密码框激活着时活动窗口换了应用：跟过去时只报能力，不报焦点、不接管键盘。
#[test]
fn following_a_password_field_only_reports_capabilities() {
    let mut session = Session::new(FakeServer::default());
    let mut out = Recorder::default();
    let now = Instant::now();
    let field = Field {
        password: true,
        ..Field::default()
    };
    session.handle(Input::Activate(field.clone()), &mut out, now);
    session.handle(app("bank"), &mut out, now);
    session.server.calls.clear();
    session.tick(&mut out, now + FOLLOW);
    assert_eq!(
        session.server.calls,
        [select("bank"), Call::Capabilities(field.capabilities())]
    );
    assert!(out.log.is_empty(), "{:?}", out.log);
}

/// 合成器不报活动窗口：一直是不知道哪个应用的那一个会话，什么都不用换。
#[test]
fn without_window_info_one_session_is_shared() {
    let (mut session, mut out, now) = ready();
    session.handle(Input::Deactivate, &mut out, now);
    session.handle(Input::Activate(Field::default()), &mut out, now);
    session.tick(&mut out, now + FOLLOW);
    assert!(
        !session
            .server
            .calls
            .iter()
            .any(|call| matches!(call, Call::Select(_))),
        "{:?}",
        session.server.calls
    );
    assert_eq!(session.server.app(), None);
}
