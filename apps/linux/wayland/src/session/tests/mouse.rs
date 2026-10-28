//! 鼠标：左键点候选上屏、滚轮翻页。请求带候选窗上那一帧最新的展示身份；过期的 Server 不理，这边也什么都不做。

use qingjian_platform::protocol::{Frame, KeyOutcome};

use super::super::{Click, Input, Spot};
use super::support::{Call, Out, answer, composing, ready, settled, type_letter};

/// 左键按在 `spot` 上。
fn left(spot: Option<Spot>) -> Input {
    Input::Click(Click {
        right: false,
        spot,
        epoch: 0,
    })
}

fn scroll(next: bool) -> Input {
    Input::Scroll { next, epoch: 0 }
}

/// 有两页的一帧。
fn paged(page: usize, candidates: &[&str]) -> Frame {
    Frame {
        page,
        page_count: 2,
        ..composing("n", candidates)
    }
}

/// 点候选：带抬键回包换上的那个身份问 Server（按下、抬起各推进一次，最新的是第 3 版），
/// 上屏与收起候选窗与按键的回包一样落地。
#[test]
fn clicking_a_candidate_commits_it() {
    let (mut session, mut out, now) = ready();
    type_letter(
        &mut session,
        &mut out,
        'n',
        composing("n", &["你", "呢"]),
        now,
    );
    settled(&mut session, &mut out, now);
    out.log.clear();
    session
        .server
        .clicks
        .push_back(answer(KeyOutcome::Consumed, Some("呢"), Frame::default()));
    session.handle(left(Some(Spot::Candidate(1))), &mut out, now);
    assert_eq!(
        out.log,
        [Out::Text("呢".to_owned(), String::new()), Out::Hide]
    );
    assert!(
        session.server.calls.contains(&Call::Candidate(1, 3)),
        "{:?}",
        session.server.calls
    );
    assert_eq!(session.deadline(), None, "收起之后不轮询");
}

/// 帧在 Server 那边已经换了：Server 回 Ignored（读成不带身份的缺省答复），候选窗与应用里的东西都不动。
#[test]
fn a_stale_click_changes_nothing() {
    let (mut session, mut out, now) = ready();
    type_letter(
        &mut session,
        &mut out,
        'n',
        composing("n", &["你", "呢"]),
        now,
    );
    settled(&mut session, &mut out, now);
    out.log.clear();
    session.server.advance();
    session.handle(left(Some(Spot::Candidate(0))), &mut out, now);
    assert!(out.log.is_empty(), "{:?}", out.log);
    assert!(session.drawn.is_some(), "候选窗还开着");
}

/// 候选窗没开、点在没有候选的地方、没弹菜单时点到菜单项：都不问 Server。
#[test]
fn clicks_on_nothing_are_ignored() {
    let (mut session, mut out, now) = ready();
    session.handle(left(Some(Spot::Candidate(0))), &mut out, now);
    type_letter(&mut session, &mut out, 'n', composing("n", &["你"]), now);
    session.server.calls.clear();
    session.handle(left(None), &mut out, now);
    session.handle(left(Some(Spot::MenuItem(0))), &mut out, now);
    session.handle(left(Some(Spot::Menu)), &mut out, now);
    assert!(
        session.server.calls.is_empty(),
        "{:?}",
        session.server.calls
    );
}

/// 点下去之后焦点换了（等回包期间点到了别的输入框）：上屏不落到新的输入框里。
#[test]
fn a_click_answered_after_the_focus_moved_is_dropped() {
    let (mut session, mut out, now) = ready();
    type_letter(&mut session, &mut out, 'n', composing("n", &["你"]), now);
    settled(&mut session, &mut out, now);
    out.log.clear();
    out.switch_on_sync = true;
    session
        .server
        .clicks
        .push_back(answer(KeyOutcome::Consumed, Some("你"), Frame::default()));
    session.handle(left(Some(Spot::Candidate(0))), &mut out, now);
    assert!(out.log.is_empty(), "{:?}", out.log);
}

/// 滚轮：一下一页，往下是下一页；只有一页时不问 Server。
#[test]
fn the_wheel_turns_pages() {
    let (mut session, mut out, now) = ready();
    type_letter(&mut session, &mut out, 'n', paged(0, &["你", "呢"]), now);
    settled(&mut session, &mut out, now);
    out.log.clear();
    session
        .server
        .clicks
        .push_back(answer(KeyOutcome::Consumed, None, paged(1, &["拟", "泥"])));
    session.handle(scroll(true), &mut out, now);
    assert_eq!(out.log, [Out::Show(vec!["拟".to_owned(), "泥".to_owned()])]);
    assert!(session.server.calls.contains(&Call::Page(true)));

    let (mut session, mut out, now) = ready();
    type_letter(&mut session, &mut out, 'n', composing("n", &["你"]), now);
    session.server.calls.clear();
    session.handle(scroll(false), &mut out, now);
    assert!(
        session.server.calls.is_empty(),
        "{:?}",
        session.server.calls
    );
}
