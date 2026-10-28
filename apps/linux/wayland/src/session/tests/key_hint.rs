//! 键位提示：Server 回包上的 `key_hint` 交给显示面，与候选窗合成进同一个 popup；候选窗收起时一并清掉。

use qingjian_platform::protocol::linux::{KeyHintContent, KeyHintView, RootChartId};
use qingjian_platform::protocol::{Frame, KeyOutcome};

use super::super::{Input, Session};
use super::support::{FakeServer, Out, Recorder, answer, composing, key, type_letter};

/// 五笔字根图的键位提示：字形随 Core 编进进程，测试里不用造双拼键位。
fn hint() -> KeyHintView {
    KeyHintView {
        content: KeyHintContent::Roots(RootChartId::Wubi86),
        opacity: 80,
        size: 100,
    }
}

/// 先打一个字母：这一键的回包就带着键位提示（键盘与候选窗合成同一张 popup）。
fn show_hint(session: &mut Session<FakeServer>, out: &mut Recorder, now: std::time::Instant) {
    session.server.key_hint = Some(hint());
    type_letter(session, out, 'n', composing("n", &["你"]), now);
}

/// 回包带上键位提示：键盘在画候选窗之前交给显示面，第一帧就与候选窗一起贴出去；
/// 帧内容没变、只是版本号变新也照发。
#[test]
fn a_reply_carries_the_key_hint_to_the_display() {
    let (mut session, mut out, now) = super::support::ready();
    show_hint(&mut session, &mut out, now);
    let hint_at = out
        .log
        .iter()
        .position(|entry| *entry == Out::KeyHint(Some(hint())))
        .unwrap_or_else(|| panic!("{:?}", out.log));
    let show_at = out
        .log
        .iter()
        .position(|entry| matches!(entry, Out::Show(_)))
        .unwrap_or_else(|| panic!("{:?}", out.log));
    assert!(
        hint_at < show_at,
        "键盘要在画候选窗之前交给显示面：{:?}",
        out.log
    );
}

/// 同一条键位提示再来一次回包不重复调用：显示面那边键盘已经画着了。
#[test]
fn the_same_hint_is_not_sent_again() {
    let (mut session, mut out, now) = super::support::ready();
    show_hint(&mut session, &mut out, now);
    out.log.clear();
    type_letter(&mut session, &mut out, 'i', composing("n", &["你"]), now);
    assert_eq!(out.log, [], "键位提示没变就不动键盘");
}

/// 回包不再带键位提示（Server 关了、或这一帧不需要）：收起键盘。
#[test]
fn a_reply_without_the_hint_clears_the_keyboard() {
    let (mut session, mut out, now) = super::support::ready();
    show_hint(&mut session, &mut out, now);
    out.log.clear();
    session.server.key_hint = None;
    type_letter(&mut session, &mut out, 'i', composing("n", &["你"]), now);
    assert_eq!(out.log, [Out::KeyHint(None)], "{:?}", out.log);
}

/// 空帧收起候选窗：键盘画在同一个 popup 里，跟着收起，会话也不再记着它。
#[test]
fn hiding_the_candidates_drops_the_keyboard() {
    let (mut session, mut out, now) = super::support::ready();
    show_hint(&mut session, &mut out, now);
    out.log.clear();
    session
        .server
        .presses
        .push_back(answer(KeyOutcome::Consumed, None, Frame::default()));
    session.handle(Input::Key(key(32, 32, true)), &mut out, now);
    assert_eq!(out.log, [Out::Hide], "{:?}", out.log);
    assert!(session.key_hint.is_none());
}
