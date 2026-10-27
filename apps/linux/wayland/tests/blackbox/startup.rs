//! 起不来与退出：别的输入法占着 seat、合成器没有 input-method-v2、收到 SIGTERM。

use super::support::{KEY_A, Request, Rig, Setup};

/// 同一个 seat 上已经有输入法（fcitx5 的 waylandim 开着）：合成器回 unavailable，退出码 1 并说明原因。
#[test]
fn unavailable_seat_exits_with_one() {
    let mut rig = Rig::spawn(Setup {
        unavailable: true,
        ..Setup::default()
    });
    let Some(exit) = rig.frontend.exit() else {
        return;
    };
    assert_eq!(exit.code, Some(1), "{}", exit.stderr);
    assert!(
        exit.stderr
            .contains("another input method already holds the seat"),
        "{}",
        exit.stderr
    );
}

/// KWin、GNOME 这类没有 input-method-v2 的合成器：退出码 1，提示改用 fcitx 版。
#[test]
fn missing_input_method_manager_exits_with_one() {
    let mut rig = Rig::spawn(Setup {
        input_method: false,
        ..Setup::default()
    });
    let Some(exit) = rig.frontend.exit() else {
        return;
    };
    assert_eq!(exit.code, Some(1), "{}", exit.stderr);
    assert!(
        exit.stderr
            .contains("the compositor does not offer zwp_input_method_manager_v2"),
        "{}",
        exit.stderr
    );
}

/// SIGTERM（systemd 停服务、会话退出）：替转给应用还按着的键补抬起、交还键盘，退出码 0。
#[test]
fn sigterm_exits_cleanly() {
    let Some(mut rig) = Rig::start(Setup::default()) else {
        return;
    };
    rig.focus();
    let mark = rig.compositor.requests.len();
    rig.compositor.press(KEY_A);
    rig.compositor
        .requests
        .wait(mark, "转给应用的按下", |request| {
            matches!(request, Request::Key { pressed: true, .. })
        });
    rig.frontend.terminate();
    let exit = rig.frontend.exit().expect("就绪过，运行库都在");
    assert_eq!(exit.code, Some(0), "{}", exit.stderr);
    rig.compositor
        .requests
        .wait(mark, "交还键盘", |request| *request == Request::Release);
    let tail: Vec<Request> = rig
        .compositor
        .requests
        .since(mark)
        .into_iter()
        .filter(|request| {
            matches!(
                request,
                Request::Key { pressed: false, .. } | Request::Release
            )
        })
        .collect();
    assert!(
        matches!(
            &tail[..],
            [
                Request::Key {
                    code: KEY_A,
                    pressed: false,
                    ..
                },
                Request::Release
            ]
        ),
        "先补抬起、再交还键盘：{tail:#?}"
    );
}
