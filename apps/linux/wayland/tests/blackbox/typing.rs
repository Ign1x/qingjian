//! 打字：输入框激活后接管键盘，Server 吃掉的字母变成行内拼音，空格上屏。

use super::support::{Answer, KEY_N, KEY_SPACE, Request, Rig, Setup, kind};

/// activate + done：先跟 Server 握手、报能力与焦点，成功了才接管键盘。
#[test]
fn activation_grabs_the_keyboard() {
    let Some(rig) = Rig::start(Setup::default()) else {
        return;
    };
    let mark = rig.compositor.requests.len();
    rig.compositor.activate();
    rig.compositor
        .requests
        .wait(mark, "接管键盘", |request| *request == Request::Grab);
    let received = rig.server.received.entries();
    let kinds: Vec<String> = received.iter().map(kind).collect();
    assert_eq!(
        kinds,
        ["OpenSession", "LinuxHello", "Capabilities", "Focus"]
    );
    assert_eq!(received[1]["LinuxHello"]["context"], "wayland");
    assert_eq!(received[3]["LinuxEvent"]["event"]["Focus"]["focused"], true);
}

/// Server 吃掉的字母：先换上行内拼音，再 commit，serial 等于合成器发过的 done 个数；这个键不转给应用。
#[test]
fn consumed_letter_sets_preedit_then_commits() {
    let Some(rig) = Rig::start(Setup::default()) else {
        return;
    };
    rig.focus();
    rig.server.answer(Answer::composing("n", &["你", "呢"]));
    let mark = rig.compositor.requests.len();
    rig.compositor.tap(KEY_N);
    rig.compositor.requests.wait(mark, "commit", |request| {
        matches!(request, Request::Commit(_))
    });
    rig.compositor.sync();
    let dones = rig.compositor.dones();
    assert_eq!(dones, 1, "只发过激活那一次 done");
    let requests = rig.compositor.requests.since(mark);
    assert_eq!(
        text(&requests),
        [
            Request::Preedit {
                text: "n".to_owned(),
                begin: 1,
                end: 1,
            },
            Request::Commit(dones),
        ]
    );
    assert!(
        !requests
            .iter()
            .any(|request| matches!(request, Request::Key { .. })),
        "吃掉的键转给了应用：{requests:#?}"
    );
    // keysym 是按 grab 给的键位表算的：Server 收到的是字母 n
    let keys = rig.server.keys();
    assert!(
        matches!(
            keys.first(),
            Some((event, false)) if event.character == Some('n') && event.virtual_key == u32::from(b'n')
        ),
        "{keys:?}"
    );
}

/// 空格选词：上屏文字先到，再清空行内拼音，同一次 commit；应用 commit 过一次（又一个 done），serial 跟着涨。
#[test]
fn space_commits_the_candidate() {
    let Some(rig) = Rig::start(Setup::default()) else {
        return;
    };
    rig.focus();
    rig.server.answer(Answer::composing("n", &["你"]));
    rig.server.answer(Answer::commit("你"));
    let mark = rig.compositor.requests.len();
    rig.compositor.tap(KEY_N);
    rig.compositor
        .requests
        .wait(mark, "行内拼音的 commit", |request| {
            matches!(request, Request::Commit(_))
        });
    // 应用处理完这次更新、commit 了自己的 text-input 状态：合成器转来一次 done
    rig.compositor.done();
    let mark = rig.compositor.requests.len();
    rig.compositor.tap(KEY_SPACE);
    rig.compositor
        .requests
        .wait(mark, "上屏的 commit", |request| {
            matches!(request, Request::Commit(_))
        });
    rig.compositor.sync();
    assert_eq!(rig.compositor.dones(), 2);
    assert_eq!(
        text(&rig.compositor.requests.since(mark)),
        [
            Request::CommitString("你".to_owned()),
            Request::Preedit {
                text: String::new(),
                begin: 0,
                end: 0,
            },
            Request::Commit(2),
        ]
    );
}

/// 只留 input-method-v2 的文字更新。
fn text(requests: &[Request]) -> Vec<Request> {
    requests
        .iter()
        .filter(|request| request.is_text())
        .cloned()
        .collect()
}
