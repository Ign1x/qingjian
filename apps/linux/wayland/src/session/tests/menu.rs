//! 候选右键菜单：右键候选问 Server 有哪几项，有就弹、停掉轮询；点一项让 Server 去做，回包与按键回包一样落地；
//! Esc 只收菜单，别的键收了菜单照常处理；点到菜单外面、换了帧、失焦都收起。

use std::time::{Duration, Instant};

use qingjian_platform::protocol::KeyOutcome;
use qingjian_platform::protocol::linux::MenuItem;
use qingjian_platform::protocol::linux::key::keysym;

use super::super::{Click, Input, POLL_INTERVAL, Session, Spot};
use super::support::{
    Call, FakeServer, Out, Recorder, answer, composing, key, ready, settled, type_letter,
};

fn click(right: bool, spot: Spot) -> Input {
    Input::Click(Click {
        right,
        spot: Some(spot),
        epoch: 0,
    })
}

fn items() -> Vec<MenuItem> {
    [(1, "置顶"), (5, "删除")]
        .into_iter()
        .map(|(id, label)| MenuItem {
            id,
            label: label.to_owned(),
        })
        .collect()
}

fn labels() -> Vec<String> {
    vec!["置顶".to_owned(), "删除".to_owned()]
}

/// 打了 n、候选窗开着、应用落定之后，右键第二个候选弹出菜单；两边的记录都清空。
fn opened() -> (Session<FakeServer>, Recorder, Instant) {
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
    session.server.menu = items();
    session.handle(click(true, Spot::Candidate(1)), &mut out, now);
    assert_eq!(out.log, [Out::Menu(labels())]);
    assert_eq!(session.server.calls.last(), Some(&Call::Menu(1)));
    session.server.calls.clear();
    out.log.clear();
    (session, out, now)
}

/// 弹着菜单时不轮询（帧换了身份就变，点菜单项会被当成过期的）；Server 给不出项、或菜单画不出来时不算弹了，轮询照旧。
#[test]
fn the_menu_stops_polling() {
    let (session, _, _) = opened();
    assert_eq!(session.deadline(), None);

    let (mut session, mut out, now) = ready();
    type_letter(&mut session, &mut out, 'n', composing("n", &["你"]), now);
    out.log.clear();
    session.handle(click(true, Spot::Candidate(0)), &mut out, now);
    assert!(out.log.is_empty(), "{:?}", out.log);
    assert_eq!(session.deadline(), Some(now + Duration::from_millis(80)));

    session.server.menu = items();
    out.menu_fails = true;
    session.handle(click(true, Spot::Candidate(0)), &mut out, now);
    assert!(session.menu.is_none());
    assert_eq!(session.deadline(), Some(now + Duration::from_millis(80)));
}

/// 点菜单里的一项：先收菜单，再带右键的那一格与那一项的编号问 Server；回包的新帧（带着提示）照常画上，接着轮询。
#[test]
fn picking_an_item_asks_the_server() {
    let (mut session, mut out, now) = opened();
    let mut after = composing("n", &["你", "呢"]);
    after.notice = Some("已删除「呢」".to_owned());
    session
        .server
        .clicks
        .push_back(answer(KeyOutcome::Consumed, None, after));
    session.handle(click(false, Spot::MenuItem(1)), &mut out, now);
    assert_eq!(
        out.log,
        [
            Out::CloseMenu,
            Out::Show(vec!["你".to_owned(), "呢".to_owned()])
        ]
    );
    assert_eq!(session.server.calls.first(), Some(&Call::MenuPick(1, 5)));
    assert_eq!(session.deadline(), Some(now + POLL_INTERVAL));
}

/// Esc 只收菜单：不报 Server，抬起也不转给应用；收了菜单接着轮询。
#[test]
fn escape_only_closes_the_menu() {
    let (mut session, mut out, now) = opened();
    session.handle(Input::Key(key(1, keysym::ESCAPE, true)), &mut out, now);
    session.handle(Input::Key(key(1, keysym::ESCAPE, false)), &mut out, now);
    assert_eq!(out.log, [Out::CloseMenu]);
    assert!(
        session.server.calls.is_empty(),
        "{:?}",
        session.server.calls
    );
    assert_eq!(session.deadline(), Some(now + POLL_INTERVAL));
}

/// 别的键：收了菜单照常交给 Server。
#[test]
fn other_keys_close_the_menu_and_go_on() {
    let (mut session, mut out, now) = opened();
    type_letter(&mut session, &mut out, 'h', composing("nh", &["你会"]), now);
    assert_eq!(out.log.first(), Some(&Out::CloseMenu));
    assert!(out.log.contains(&Out::Show(vec!["你会".to_owned()])));
    assert!(matches!(
        session.server.calls.first(),
        Some(Call::Key { release: false, .. })
    ));
}

/// 点到菜单外面只收菜单、不选词；点在菜单的边上不收；右键另一个候选换成它的菜单。
#[test]
fn clicks_outside_the_menu_close_it() {
    let (mut session, mut out, now) = opened();
    session.handle(click(false, Spot::Menu), &mut out, now);
    assert!(out.log.is_empty(), "{:?}", out.log);

    session.handle(click(true, Spot::Candidate(0)), &mut out, now);
    assert_eq!(out.log, [Out::CloseMenu, Out::Menu(labels())]);
    assert_eq!(session.server.calls, [Call::Menu(0)]);
    out.log.clear();

    session.handle(click(false, Spot::Candidate(1)), &mut out, now);
    assert_eq!(out.log, [Out::CloseMenu]);
    assert_eq!(session.server.calls, [Call::Menu(0)], "没有选词");
    assert!(session.menu.is_none());
}

/// 菜单开着时滚轮不翻页；输入框失活时菜单随候选窗一起收起。
#[test]
fn the_wheel_and_focus_loss() {
    let (mut session, mut out, now) = opened();
    session.handle(
        Input::Scroll {
            next: true,
            epoch: 0,
        },
        &mut out,
        now,
    );
    assert!(
        session.server.calls.is_empty(),
        "{:?}",
        session.server.calls
    );
    session.handle(Input::Deactivate, &mut out, now);
    assert_eq!(out.log, [Out::Hide]);
    assert!(session.menu.is_none());
}

/// 菜单开着时抬起了之前按下的键：Server 回的帧没变、身份换了，菜单留着，点菜单项带的是新身份（带旧的会被当成过期）。
#[test]
fn a_release_keeps_the_menu_with_the_new_identity() {
    let (mut session, mut out, now) = ready();
    session.server.presses.push_back(answer(
        KeyOutcome::Consumed,
        None,
        composing("n", &["你", "呢"]),
    ));
    session.handle(Input::Key(key(49, 'n' as u32, true)), &mut out, now);
    settled(&mut session, &mut out, now);
    session.server.menu = items();
    session.handle(click(true, Spot::Candidate(0)), &mut out, now);
    out.log.clear();

    session.handle(Input::Key(key(49, 'n' as u32, false)), &mut out, now);
    assert!(out.log.is_empty(), "帧没变，不重画：{:?}", out.log);
    assert!(session.menu.is_some());
    session.server.clicks.push_back(answer(
        KeyOutcome::Consumed,
        None,
        composing("n", &["呢", "你"]),
    ));
    session.handle(click(false, Spot::MenuItem(0)), &mut out, now);
    assert!(
        out.log
            .contains(&Out::Show(vec!["呢".to_owned(), "你".to_owned()])),
        "{:?}",
        out.log
    );
    assert!(session.server.calls.contains(&Call::MenuPick(0, 1)));
}
