//! 放行的键：经虚拟键盘原样转给应用；失活时还按着的键，等它抬起再交还键盘。

use super::support::{KEY_A, KEYMAP, Request, Rig, Setup, kind};

/// Server 放行的字母：虚拟键盘先拿到与 grab 一字不差的键位表，再收到这个键的按下与抬起，时间戳原样；
/// 按下与抬起都问过 Server（单击 Shift 切中英认的就是抬起）。
#[test]
fn passthrough_key_goes_through_the_virtual_keyboard() {
    let Some(rig) = Rig::start(Setup::default()) else {
        return;
    };
    let mark = rig.compositor.requests.len();
    rig.focus();
    let pressed = rig.compositor.press(KEY_A);
    let released = rig.compositor.release(KEY_A);
    rig.compositor
        .requests
        .wait(mark, "转给应用的抬起", |request| {
            matches!(request, Request::Key { pressed: false, .. })
        });
    let forwarded: Vec<Request> = rig
        .compositor
        .requests
        .since(mark)
        .into_iter()
        .filter(|request| matches!(request, Request::Keymap(_) | Request::Key { .. }))
        .collect();
    assert_eq!(
        forwarded,
        [
            Request::Keymap(KEYMAP.to_owned()),
            Request::Key {
                code: KEY_A,
                pressed: true,
                time: pressed,
            },
            Request::Key {
                code: KEY_A,
                pressed: false,
                time: released,
            },
        ]
    );
    let keys: Vec<(Option<char>, bool)> = rig
        .server
        .keys()
        .iter()
        .map(|(event, release)| (event.character, *release))
        .collect();
    assert_eq!(keys, [(Some('a'), false), (Some('a'), true)]);
}

/// 失活时还按着转给应用的键：先不交还键盘，等这个键抬起（照样转给应用）之后才交还，
/// 免得抬键落到新焦点上、虚拟键盘上这个键一直按着。
#[test]
fn deactivation_keeps_the_grab_until_held_keys_are_released() {
    let Some(rig) = Rig::start(Setup::default()) else {
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
    let mark = rig.compositor.requests.len();
    rig.compositor.deactivate();
    rig.server
        .received
        .wait(0, "失活", |message| kind(message) == "Deactivate");
    rig.compositor.sync();
    assert!(
        !rig.compositor
            .requests
            .since(mark)
            .contains(&Request::Release),
        "键还按着就交还了键盘"
    );
    let released = rig.compositor.release(KEY_A);
    rig.compositor
        .requests
        .wait(mark, "交还键盘", |request| *request == Request::Release);
    let tail: Vec<Request> = rig
        .compositor
        .requests
        .since(mark)
        .into_iter()
        .filter(|request| matches!(request, Request::Key { .. } | Request::Release))
        .collect();
    assert_eq!(
        tail,
        [
            Request::Key {
                code: KEY_A,
                pressed: false,
                time: released,
            },
            Request::Release,
        ]
    );
}
