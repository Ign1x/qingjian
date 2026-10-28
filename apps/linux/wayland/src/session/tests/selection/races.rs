//! 选区请求与上屏排队期间，输入框变化不能泄漏旧文字或覆盖新位置。

use qingjian_platform::protocol::{Frame, KeyOutcome};

use super::support::{T, T_KEYSYM, field, selecting, start_translating};
use crate::session::tests::support::{Call, Out, answer, composing, key, ready, type_letter};
use crate::session::{Field, Input, SETTLE};

#[test]
fn a_stale_selection_request_is_answered_without_text() {
    let (mut session, mut out, now) = ready();
    session.handle(Input::Update(selecting()), &mut out, now);
    session.server.selection_request = Some(7);
    out.switch_on_sync = true;
    session.handle(Input::Key(key(T, T_KEYSYM, true)), &mut out, now);
    assert_eq!(
        session.server.calls.last(),
        Some(&Call::Selection(7, String::new()))
    );
    assert_eq!(out.log, [Out::Forward(T, true)]);
}

#[test]
fn an_external_edit_resets_even_an_unshown_translation() {
    let (mut session, mut out, now) = ready();
    session.handle(Input::Update(selecting()), &mut out, now);
    start_translating(&mut session.server);
    session.handle(Input::Key(key(T, T_KEYSYM, true)), &mut out, now);
    // 模拟回包因合成器已报新状态而没画出来；Server 上的翻译仍需取消。
    session.drawn = None;
    session.handle(
        Input::Update(Field {
            external_change: true,
            ..field("我喜欢苹果。", 6, 0)
        }),
        &mut out,
        now,
    );
    assert_eq!(session.server.calls.last(), Some(&Call::Reset));
}

#[test]
fn invalidating_the_field_discards_buffered_commits() {
    for changed in [
        Field {
            password: true,
            ..selecting()
        },
        Field {
            sensitive: true,
            ..selecting()
        },
        Field {
            external_change: true,
            ..field("我喜欢苹果。", 6, 0)
        },
    ] {
        let (mut session, mut out, now) = ready();
        session.handle(Input::Update(selecting()), &mut out, now);
        type_letter(&mut session, &mut out, 'n', composing("n", &["你"]), now);
        session.server.presses.push_back(answer(
            KeyOutcome::Consumed,
            Some("apple"),
            Frame::default(),
        ));
        session.handle(Input::Key(key(57, ' ' as u32, true)), &mut out, now);
        assert!(session.unsent.dirty, "上一次文字更新尚未落定，上屏应在排队");
        out.log.clear();
        session.handle(Input::Update(changed), &mut out, now);
        session.tick(&mut out, now + SETTLE);
        assert!(
            !out.log
                .iter()
                .any(|entry| matches!(entry, Out::Text(commit, _) if !commit.is_empty())),
            "{:?}",
            out.log
        );
    }
}
