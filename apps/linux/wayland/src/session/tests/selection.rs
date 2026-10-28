//! 选区翻译：有选区时按下报能读选区，Server 要选区就回选中的文字，之后的回包像按键一样落地。

use std::time::Duration;

use qingjian_core::{Candidate, CandidateKind, CandidateList};
use qingjian_platform::protocol::{Frame, KeyOutcome};

use super::super::{Field, Input, Surrounding};
use super::support::{Call, FakeServer, Out, answer, key, ready};

/// T 的 evdev 键码；keysym 给大写的 T（按着 Shift），Server 收到的虚拟键码也是它。
const T: u32 = 20;

const T_KEYSYM: u32 = 'T' as u32;

/// 「我喜欢苹果。」里选中「苹果」（9 到 15 字节），光标在选区后头。
fn selecting() -> Field {
    field("我喜欢苹果。", 15, 9)
}

fn field(text: &str, cursor: usize, anchor: usize) -> Field {
    Field {
        surrounding: Some(Surrounding {
            text: text.to_owned(),
            cursor,
            anchor,
        }),
        ..Field::default()
    }
}

/// 只有一条候选、没有拼音的帧，与 Server 翻译时给的一样。
fn single(text: &str) -> Frame {
    Frame {
        candidates: CandidateList {
            items: vec![Candidate {
                text: text.to_owned(),
                kind: CandidateKind::Cloud,
                syllables: Vec::new(),
                reading: None,
                translation: None,
                aux_code: None,
            }],
        },
        page_count: 1,
        ..Frame::default()
    }
}

/// 按下翻译快捷键，Server 要选区、回了选区就开始翻译：候选窗是「翻译中…」，回包说还在等译文。
fn start_translating(server: &mut FakeServer) {
    server.selection_request = Some(7);
    server.pending = true;
    server
        .presses
        .push_back(answer(KeyOutcome::Consumed, None, single("翻译中…")));
}

/// 按下时报的「能读选区」，按先后。
fn reported(server: &FakeServer) -> Vec<bool> {
    server
        .calls
        .iter()
        .filter_map(|call| match call {
            Call::Key {
                release: false,
                selection,
                ..
            } => Some(*selection),
            _ => None,
        })
        .collect()
}

/// 前后文里有选区才报能读选区；没有选区、敏感输入框都不报（Server 把翻译快捷键当普通键）。
#[test]
fn only_a_readable_selection_is_reported() {
    let (mut session, mut out, now) = ready();
    let fields = [
        field("今天", 6, 6),
        selecting(),
        Field {
            sensitive: true,
            ..selecting()
        },
    ];
    for field in fields {
        session.handle(Input::Update(field), &mut out, now);
        session.handle(Input::Key(key(T, T_KEYSYM, true)), &mut out, now);
        session.handle(Input::Key(key(T, T_KEYSYM, false)), &mut out, now);
    }
    assert_eq!(reported(&session.server), [false, true, false]);
}

/// 一整轮：回选中的「苹果」；「翻译中…」时不放拼音、应用里的选区不动，按住也不重复，接着轮询；
/// 译文到了重画，空格把译文上屏（应用用它替换选区）。
#[test]
fn the_selected_text_is_translated_and_committed() {
    let (mut session, mut out, now) = ready();
    session.handle(Input::Update(selecting()), &mut out, now);
    start_translating(&mut session.server);
    session.handle(Input::Key(key(T, T_KEYSYM, true)), &mut out, now);
    assert!(
        matches!(
            &session.server.calls[..],
            [
                Call::Key { selection: true, .. },
                Call::Selection(7, text),
                Call::Acknowledge(_),
            ] if text == "苹果"
        ),
        "{:?}",
        session.server.calls
    );
    assert_eq!(out.log, [Out::Show(vec!["翻译中…".to_owned()])]);

    let held = now + Duration::from_millis(700);
    session.tick(&mut out, held);
    let presses = session
        .server
        .calls
        .iter()
        .filter(|call| matches!(call, Call::Key { release: false, .. }))
        .count();
    assert_eq!(presses, 1, "翻译快捷键按住不重复");
    assert_eq!(session.server.calls.last(), Some(&Call::Poll));
    assert_eq!(out.log.len(), 1, "译文没到，帧没变不重画");
    session.handle(Input::Key(key(T, T_KEYSYM, false)), &mut out, held);
    assert!(
        !out.log
            .iter()
            .any(|entry| matches!(entry, Out::Forward(..))),
        "吃掉的键不转给应用：{:?}",
        out.log
    );

    session.server.pending = false;
    session.server.polled = Some(single("apple"));
    let later = held + Duration::from_millis(80);
    session.tick(&mut out, later);
    assert_eq!(out.log.last(), Some(&Out::Show(vec!["apple".to_owned()])));

    session.server.presses.push_back(answer(
        KeyOutcome::Consumed,
        Some("apple"),
        Frame::default(),
    ));
    out.log.clear();
    session.handle(Input::Key(key(57, ' ' as u32, true)), &mut out, later);
    assert_eq!(
        out.log,
        [Out::Text("apple".to_owned(), String::new()), Out::Hide]
    );
}

/// 选区读不了（超过 500 字、偏移落在字符中间）：回空串，Server 放行，快捷键原样交给应用。
#[test]
fn an_unreadable_selection_hands_the_key_to_the_app() {
    let long = "字".repeat(501);
    for field in [field(&long, long.len(), 0), field("我喜欢苹果。", 10, 3)] {
        let (mut session, mut out, now) = ready();
        session.handle(Input::Update(field), &mut out, now);
        session.server.selection_request = Some(8);
        session.handle(Input::Key(key(T, T_KEYSYM, true)), &mut out, now);
        assert_eq!(
            session.server.calls.last(),
            Some(&Call::Selection(8, String::new()))
        );
        assert_eq!(out.log, [Out::Forward(T, true)]);
    }
}

/// 敏感输入框不报能读选区；Server 真来要（不该发生）也只回空串。
#[test]
fn a_sensitive_field_never_hands_over_its_selection() {
    let (mut session, mut out, now) = ready();
    let field = Field {
        sensitive: true,
        ..selecting()
    };
    session.handle(Input::Update(field), &mut out, now);
    session.answer_selection(9).expect("回选区");
    assert_eq!(
        session.server.calls.last(),
        Some(&Call::Selection(9, String::new()))
    );
}

/// 翻译中应用自己改了选区（鼠标点到别处）：与组句中一样报 Reset、收起候选窗，译文不会替换到别的地方。
#[test]
fn moving_the_selection_cancels_the_translation() {
    let (mut session, mut out, now) = ready();
    session.handle(Input::Update(selecting()), &mut out, now);
    start_translating(&mut session.server);
    session.handle(Input::Key(key(T, T_KEYSYM, true)), &mut out, now);
    out.log.clear();
    let moved = Field {
        external_change: true,
        ..field("我喜欢苹果。", 6, 0)
    };
    session.handle(Input::Update(moved), &mut out, now);
    assert_eq!(session.server.calls.last(), Some(&Call::Reset));
    assert_eq!(out.log, [Out::Hide]);
}
