//! 组句、上屏、候选窗、轮询与回执。

use std::time::Duration;

use qingjian_platform::protocol::{Frame, KeyOutcome};

use super::super::{Input, SETTLE};
use super::support::{Call, Out, answer, composing, key, ready, settled, type_letter};

/// nihao 组句：行内拼音与候选窗跟着变；抬键的回包帧没变不重画；空格上屏后收起。
#[test]
fn typing_composes_and_commits() {
    let (mut session, mut out, now) = ready();
    type_letter(
        &mut session,
        &mut out,
        'n',
        composing("n", &["你", "呢"]),
        now,
    );
    assert_eq!(
        out.log,
        [
            Out::Text(String::new(), "n".to_owned()),
            Out::Show(vec!["你".to_owned(), "呢".to_owned()]),
        ],
        "按下吃掉：放进拼音、开候选窗；抬起帧没变，什么都不做，也不转给应用"
    );
    out.log.clear();
    settled(&mut session, &mut out, now);

    session
        .server
        .presses
        .push_back(answer(KeyOutcome::Consumed, Some("你"), Frame::default()));
    session.handle(Input::Key(key(32, 32, true)), &mut out, now);
    assert_eq!(
        out.log,
        [Out::Text("你".to_owned(), String::new()), Out::Hide],
        "上屏与清掉拼音是同一次 commit"
    );
    // 每画一帧都回执，抬键的回包也回（展示身份换了）
    let acks = session
        .server
        .calls
        .iter()
        .filter(|call| matches!(call, Call::Acknowledge(_)))
        .count();
    assert_eq!(acks, 2);
}

/// 组句中 Ctrl+A：Server 放行并交回原文，先上屏再把键交给应用。
#[test]
fn shortcut_flush_commits_before_the_key_reaches_the_app() {
    let (mut session, mut out, now) = ready();
    type_letter(&mut session, &mut out, 'n', composing("ni", &["你"]), now);
    out.log.clear();
    session.server.presses.push_back(answer(
        KeyOutcome::Passthrough,
        Some("ni"),
        Frame::default(),
    ));
    session.handle(Input::Key(key(30, 'a' as u32, true)), &mut out, now);
    assert_eq!(
        out.log,
        [
            Out::Hide,
            Out::Text("ni".to_owned(), String::new()),
            Out::Forward(30, true)
        ],
        "上一次更新应用还没落定也不等：先上屏再给键"
    );
}

/// 等回包期间焦点换了：这次的上屏与候选不落地，放行的键照转。
#[test]
fn a_stale_reply_does_not_reach_the_new_field() {
    let (mut session, mut out, now) = ready();
    out.switch_on_sync = true;
    session.server.presses.push_back(answer(
        KeyOutcome::Consumed,
        Some("你"),
        composing("h", &["和"]),
    ));
    session.handle(Input::Key(key(35, 'h' as u32, true)), &mut out, now);
    assert!(out.log.is_empty(), "{:?}", out.log);

    out.switch_on_sync = true;
    session
        .server
        .presses
        .push_back(answer(KeyOutcome::Passthrough, None, Frame::default()));
    session.handle(Input::Key(key(36, 'j' as u32, true)), &mut out, now);
    assert_eq!(out.log, [Out::Forward(36, true)]);
}

/// 超过 4000 字节的上屏按字符边界切块，拼音只跟在最后一块上。
#[test]
fn a_long_commit_is_sent_in_chunks() {
    let (mut session, mut out, now) = ready();
    let long = "字".repeat(3000);
    session.server.presses.push_back(answer(
        KeyOutcome::Consumed,
        Some(&long),
        composing("x", &["新"]),
    ));
    session.handle(Input::Key(key(45, 'x' as u32, true)), &mut out, now);
    let texts: Vec<_> = out
        .log
        .iter()
        .filter_map(|entry| match entry {
            Out::Text(commit, preedit) => Some((commit.len(), preedit.as_str())),
            _ => None,
        })
        .collect();
    assert_eq!(texts, [(3999, ""), (3999, ""), (1002, "x")]);
}

/// 组句期间轮询：版本号没变不重画；重排结果到了重画并回执。
#[test]
fn polling_redraws_only_newer_frames() {
    let (mut session, mut out, now) = ready();
    type_letter(
        &mut session,
        &mut out,
        'n',
        composing("nihao", &["你好"]),
        now,
    );
    out.log.clear();
    let later = now + Duration::from_millis(80);
    assert_eq!(session.deadline(), Some(later));

    session.tick(&mut out, later);
    assert!(out.log.is_empty(), "帧没变：{:?}", out.log);

    session.server.polled = Some(composing("nihao", &["你好", "拟好"]));
    let later = later + Duration::from_millis(80);
    session.tick(&mut out, later);
    assert_eq!(
        out.log,
        [Out::Show(vec!["你好".to_owned(), "拟好".to_owned()])]
    );
    assert!(matches!(
        session.server.calls.last(),
        Some(Call::Acknowledge(_))
    ));
}

/// 每个键都带上光标前后文；敏感输入框不带。
#[test]
fn keys_carry_the_surrounding_text_unless_sensitive() {
    use super::super::{Field, Surrounding};

    let (mut session, mut out, now) = ready();
    let field = Field {
        surrounding: Some(Surrounding {
            text: "今天".to_owned(),
            cursor: 6,
            anchor: 6,
        }),
        ..Field::default()
    };
    session.handle(Input::Update(field.clone()), &mut out, now);
    session.handle(Input::Key(key(49, 'n' as u32, true)), &mut out, now);
    assert!(matches!(
        session.server.calls.last(),
        Some(Call::Key { context: true, .. })
    ));
    session.handle(
        Input::Update(super::super::Field {
            sensitive: true,
            ..field
        }),
        &mut out,
        now,
    );
    session.handle(Input::Key(key(50, 'm' as u32, true)), &mut out, now);
    assert!(matches!(
        session.server.calls.last(),
        Some(Call::Key { context: false, .. })
    ));
}

/// 上一次更新应用还没落定（没收到 `done`）时攒着，落定或等满 SETTLE 再发最新的一份：
/// foot 这类应用收到 serial 对不上的 `done` 会把整次更新丢掉。
#[test]
fn text_updates_wait_for_the_client_to_settle() {
    let (mut session, mut out, now) = ready();
    type_letter(&mut session, &mut out, 'n', composing("n", &["你"]), now);
    out.log.clear();
    type_letter(&mut session, &mut out, 'i', composing("ni", &["你"]), now);
    assert!(
        !out.log.iter().any(|entry| matches!(entry, Out::Text(..))),
        "{:?}",
        out.log
    );
    assert_eq!(session.deadline(), Some(now + SETTLE));

    settled(&mut session, &mut out, now);
    assert_eq!(
        out.log.last(),
        Some(&Out::Text(String::new(), "ni".to_owned()))
    );
    out.log.clear();

    type_letter(&mut session, &mut out, 'h', composing("ni'h", &["你"]), now);
    session.tick(&mut out, now + SETTLE);
    assert_eq!(
        out.log.last(),
        Some(&Out::Text(String::new(), "ni'h".to_owned()))
    );
}

/// 攒着期间又上屏了两次：拼在一起一次发出去，不丢。
#[test]
fn pending_commits_are_joined() {
    let (mut session, mut out, now) = ready();
    type_letter(&mut session, &mut out, 'n', composing("ni", &["你"]), now);
    for text in ["你", "好"] {
        session.server.presses.push_back(answer(
            KeyOutcome::Consumed,
            Some(text),
            composing("x", &["新"]),
        ));
        session.handle(Input::Key(key(2, '1' as u32, true)), &mut out, now);
    }
    out.log.clear();
    settled(&mut session, &mut out, now);
    assert_eq!(out.log, [Out::Text("你好".to_owned(), "x".to_owned())]);
}
