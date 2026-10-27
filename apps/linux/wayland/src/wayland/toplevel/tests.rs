//! 活动窗口的跟踪：`done` 时生效、激活先后、关窗口、晚报或改了的 app_id。

use super::{Toplevels, activated};

/// 一个窗口报齐 app_id 与状态，再 `done`。
fn open(toplevels: &mut Toplevels, id: u32, app: &str, active: bool) {
    toplevels.app_id(id, app.to_owned());
    toplevels.state(id, active);
    toplevels.done(id);
}

fn activate(toplevels: &mut Toplevels, id: u32, active: bool) {
    toplevels.state(id, active);
    toplevels.done(id);
}

fn app(name: &str) -> Option<Option<String>> {
    Some(Some(name.to_owned()))
}

/// 窗口的变化在 `done` 时才整份生效；激活着的窗口的 app_id 就是活动窗口的应用，变了才报，没变不重复报。
#[test]
fn the_activated_window_is_the_app() {
    let mut toplevels = Toplevels::default();
    assert_eq!(toplevels.changed(), None, "一开始就是不知道哪个应用");
    toplevels.app_id(1, "firefox".to_owned());
    toplevels.state(1, true);
    assert_eq!(toplevels.changed(), None, "done 之前不算数");
    toplevels.done(1);
    assert_eq!(toplevels.changed(), app("firefox"));
    assert_eq!(toplevels.changed(), None);
    open(&mut toplevels, 2, "foot", false);
    assert_eq!(toplevels.changed(), None, "没激活的窗口不算");
}

/// 焦点换窗口：新窗口先报激活、旧窗口后报失活，或者反过来，最后都落在新窗口上。
/// 先失活的那种中间有一下没有活动窗口，照实报（会话那边等一会儿再跟）。
#[test]
fn focus_moves_between_windows_in_either_order() {
    let mut toplevels = Toplevels::default();
    open(&mut toplevels, 1, "firefox", true);
    open(&mut toplevels, 2, "foot", false);
    toplevels.changed();

    activate(&mut toplevels, 2, true);
    assert_eq!(toplevels.changed(), app("foot"), "两个都激活着时取后激活的");
    activate(&mut toplevels, 1, false);
    assert_eq!(toplevels.changed(), None);

    activate(&mut toplevels, 2, false);
    assert_eq!(toplevels.changed(), Some(None));
    activate(&mut toplevels, 1, true);
    assert_eq!(toplevels.changed(), app("firefox"));
}

/// 激活着的窗口又报一次激活（最大化之类改了别的状态）不算重新激活，抢不过后激活的窗口。
#[test]
fn repeated_states_keep_the_activation_order() {
    let mut toplevels = Toplevels::default();
    open(&mut toplevels, 1, "firefox", true);
    open(&mut toplevels, 2, "foot", true);
    assert_eq!(toplevels.changed(), app("foot"));
    activate(&mut toplevels, 1, true);
    assert_eq!(toplevels.changed(), None);
}

/// 活动窗口晚报 app_id、改了 app_id、关掉；app_id 是空串当没报。
#[test]
fn app_ids_can_arrive_late_change_and_go_away() {
    let mut toplevels = Toplevels::default();
    open(&mut toplevels, 1, "", true);
    assert_eq!(toplevels.changed(), None, "空串当不知道哪个应用");
    toplevels.app_id(1, "org.gnome.TextEditor".to_owned());
    toplevels.done(1);
    assert_eq!(toplevels.changed(), app("org.gnome.TextEditor"));
    toplevels.app_id(1, "org.gnome.Nautilus".to_owned());
    toplevels.done(1);
    assert_eq!(toplevels.changed(), app("org.gnome.Nautilus"));
    toplevels.closed(1);
    assert_eq!(toplevels.changed(), Some(None));
}

/// `state` 的数组是本机字节序的 u32 一串，activated 是 2，与别的状态混在一起也认得出。
#[test]
fn the_state_array_is_read_as_native_u32s() {
    let states = |values: &[u32]| -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| value.to_ne_bytes())
            .collect()
    };
    assert!(activated(&states(&[2])));
    assert!(activated(&states(&[0, 3, 2])), "最大化、全屏又激活着");
    assert!(!activated(&states(&[0, 1, 3])));
    assert!(!activated(&[]));
}
