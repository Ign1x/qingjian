//! 鼠标请求等待期间的焦点变化与 Server 断开，不能把旧菜单或文字留给新输入框。

use std::time::Instant;

use qingjian_platform::protocol::linux::MenuItem;

use crate::session::tests::support::{
    FakeServer, Out, Recorder, composing, ready, settled, type_letter,
};
use crate::session::{Click, Input, Session, Spot};

fn click(right: bool, spot: Spot) -> Input {
    Input::Click(Click {
        right,
        spot: Some(spot),
        position: (0.0, 0.0),
        epoch: 0,
    })
}

fn composing_session() -> (Session<FakeServer>, Recorder, Instant) {
    let (mut session, mut out, now) = ready();
    let mut frame = composing("n", &["你", "呢"]);
    frame.page_count = 2;
    type_letter(&mut session, &mut out, 'n', frame, now);
    settled(&mut session, &mut out, now);
    session.server.menu = vec![MenuItem {
        id: 5,
        label: "删除".to_owned(),
    }];
    out.log.clear();
    (session, out, now)
}

#[test]
fn a_menu_answered_after_the_focus_moved_is_dropped() {
    let (mut session, mut out, now) = composing_session();
    out.switch_on_sync = true;
    session.handle(click(true, Spot::Candidate(0)), &mut out, now);
    assert_eq!(out.epoch, 2, "菜单回包也要读一遍焦点变化");
    assert!(out.log.is_empty(), "旧输入框的菜单不应弹出：{:?}", out.log);
    assert!(session.menu.is_none());
}

#[test]
fn queued_mouse_requests_after_focus_change_are_ignored() {
    for input in [
        click(false, Spot::Candidate(0)),
        click(true, Spot::Candidate(0)),
        click(false, Spot::MenuItem(0)),
        Input::Scroll {
            next: true,
            epoch: 0,
        },
    ] {
        let (mut session, mut out, now) = composing_session();
        if matches!(
            input,
            Input::Click(Click {
                spot: Some(Spot::MenuItem(_)),
                ..
            })
        ) {
            session.handle(click(true, Spot::Candidate(0)), &mut out, now);
        }
        session.server.calls.clear();
        out.log.clear();
        out.epoch = 2;
        session.handle(input, &mut out, now);
        assert!(
            session.server.calls.is_empty(),
            "焦点已换，不能再向 Server 发旧操作"
        );
        assert!(out.log.is_empty());
    }
}

#[test]
fn mouse_connection_failures_hide_the_popup_and_release_the_grab() {
    for input in [
        click(false, Spot::Candidate(0)),
        click(true, Spot::Candidate(0)),
        click(false, Spot::MenuItem(0)),
        Input::Scroll {
            next: true,
            epoch: 0,
        },
    ] {
        let (mut session, mut out, now) = composing_session();
        if matches!(
            input,
            Input::Click(Click {
                spot: Some(Spot::MenuItem(_)),
                ..
            })
        ) {
            session.handle(click(true, Spot::Candidate(0)), &mut out, now);
        }
        out.log.clear();
        session.server.down = true;
        session.handle(input, &mut out, now);
        session.tick(&mut out, now);
        assert!(out.log.contains(&Out::Hide));
        assert!(out.log.contains(&Out::Grab(false)));
        assert!(session.menu.is_none());
        assert!(session.identity.is_none());
        assert!(session.poll_at.is_none());
    }
}
