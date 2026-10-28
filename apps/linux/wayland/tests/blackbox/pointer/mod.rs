//! 鼠标：往候选窗发 enter / motion / button / axis——点候选上屏、滚轮翻页、右键弹菜单与点菜单里的一项、Esc 收菜单。
//! 点哪儿按被测进程调试日志里记的可点位置（surface 坐标）找。合成器肯不肯把指针给输入法的 popup 要在真会话里验。

use std::thread;
use std::time::Duration;

mod backpressure;
mod races;

use crate::support::{
    Answer, BUTTON_LEFT, BUTTON_RIGHT, KEY_ESC, KEY_N, MENU, Request, Rig, Setup, Targets, kind,
    skip,
};

/// 前端认为点选过期、Server 没理时记的那句日志（`src/session/mouse/mod.rs`）。
const STALE: &str = "这一下 Server 没理";

/// 起好、接管键盘，打一个 n 让候选窗按 `answer` 画出来，交出能点的地方。缺运行库或界面字体时 `None`（已打印跳过）。
pub(super) fn composing(answer: Answer) -> Option<(Rig, Targets)> {
    let rig = Rig::start(Setup::default())?;
    if !rig.frontend.painter() {
        skip("这台机器上没有界面字体，候选窗画不出来");
        return None;
    }
    rig.focus();
    rig.server.answer(answer);
    let mark = rig.frontend.mark();
    rig.compositor.tap(KEY_N);
    let targets = rig.frontend.targets(mark);
    Some((rig, targets))
}

/// 左键点第二个候选：进候选窗时光标设成默认的箭头；带着画这一页的展示身份问 Server，上屏它、候选窗收起。
#[test]
fn clicking_a_candidate_commits_it() {
    let Some((rig, targets)) = composing(Answer::composing("n", &["你", "呢"])) else {
        return;
    };
    rig.server.answer(Answer::commit("呢"));
    let (x, y) = targets.at("candidate1");
    let mark = rig.compositor.requests.len();
    rig.compositor.enter(x, y);
    rig.compositor.requests.wait(mark, "设光标形状", |request| {
        matches!(request, Request::CursorShape { shape: 1, .. })
    });
    rig.compositor.click(BUTTON_LEFT);
    rig.compositor
        .requests
        .wait(mark, "点中的候选上屏", |request| {
            *request == Request::CommitString("呢".to_owned())
        });
    rig.compositor.requests.wait(mark, "候选窗收起", |request| {
        matches!(request, Request::Popup { picture: None, .. })
    });
    let clicks = rig.server.events("Candidate");
    assert_eq!(clicks.len(), 1, "{clicks:?}");
    assert_eq!(clicks[0]["index"], 1);
    assert!(!rig.frontend.logged(STALE));
}

/// 滚轮一格翻一页：同一下的格数与距离只算一次。触摸板按滑过的距离攒，够了才翻，往上滑回上一页。
#[test]
fn the_wheel_turns_one_page_per_notch() {
    let first = Answer::composing("n", &["你", "呢"]).on_page(0, 2);
    let Some((rig, targets)) = composing(first) else {
        return;
    };
    rig.server
        .answer(Answer::composing("n", &["拟", "泥"]).on_page(1, 2));
    rig.server
        .answer(Answer::composing("n", &["你", "呢"]).on_page(0, 2));
    let (x, y) = targets.at("candidate0");
    rig.compositor.enter(x, y);
    rig.compositor.wheel(120);
    rig.compositor.sync();
    let pages = rig.server.events("Page");
    assert_eq!(pages.len(), 1, "一格只翻一页：{pages:?}");
    assert_eq!(pages[0]["next"], true);

    rig.compositor.swipe(-20.0);
    rig.compositor.swipe(-20.0);
    rig.compositor.sync();
    assert_eq!(rig.server.events("Page").len(), 1, "滑得还不够一页");
    rig.compositor.swipe(-20.0);
    rig.compositor.sync();
    let pages = rig.server.events("Page");
    assert_eq!(pages.len(), 2, "{pages:?}");
    assert_eq!(pages[1]["next"], false);
    assert!(!rig.frontend.logged(STALE), "翻页带的是过期的身份");
}

/// 右键第二个候选弹出菜单，菜单开着时不轮询；鼠标移到第二项上、左键点下去：
/// 带着右键的那一格、那一项的编号与弹菜单时的展示身份问 Server。
#[test]
fn right_click_opens_the_menu_and_picks_an_item() {
    let Some((rig, targets)) = composing(Answer::composing("n", &["你", "呢"])) else {
        return;
    };
    rig.server.answer(Answer::composing("n", &["呢", "你"]));
    let (x, y) = targets.at("candidate1");
    rig.compositor.enter(x, y);
    let mark = rig.frontend.mark();
    rig.compositor.click(BUTTON_RIGHT);
    let menu = rig.frontend.targets(mark);
    let asked = rig.server.events("Menu");
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert_eq!(asked[0]["index"], 1);

    // 等过好几个轮询间隔（80 毫秒）：问过菜单之后一次 Poll 也不该有（弹菜单之前排着的那次也取消了）
    thread::sleep(Duration::from_millis(300));
    let kinds: Vec<String> = rig.server.received.entries().iter().map(kind).collect();
    let opened = kinds
        .iter()
        .position(|name| name == "Menu")
        .expect("问过菜单");
    assert!(
        !kinds[opened..].iter().any(|name| name == "Poll"),
        "菜单开着时还在轮询：{kinds:?}"
    );

    let (x, y) = menu.at("item1");
    rig.compositor.motion(x, y);
    rig.compositor.click(BUTTON_LEFT);
    rig.server
        .received
        .wait(0, "点菜单项", |message| kind(message) == "MenuPick");
    let picked = rig.server.events("MenuPick");
    assert_eq!(picked[0]["index"], 1);
    assert_eq!(picked[0]["action"], MENU[1].0);
    assert_eq!(picked[0]["identity"], asked[0]["identity"]);
    rig.compositor.sync();
    assert!(!rig.frontend.logged(STALE), "点菜单项带的是过期的身份");
}

/// 菜单开着时按 Esc：只收起菜单，这个键不报 Server、也不转给应用；收起之后接着轮询。
#[test]
fn escape_closes_only_the_menu() {
    let Some((rig, targets)) = composing(Answer::composing("n", &["你", "呢"])) else {
        return;
    };
    let (x, y) = targets.at("candidate0");
    rig.compositor.enter(x, y);
    let mark = rig.frontend.mark();
    rig.compositor.click(BUTTON_RIGHT);
    assert!(rig.frontend.targets(mark).has("item0"), "菜单没弹出来");

    let keys = rig.server.keys().len();
    let mark = rig.frontend.mark();
    let requests = rig.compositor.requests.len();
    rig.compositor.tap(KEY_ESC);
    assert!(!rig.frontend.targets(mark).has("item0"), "菜单没收起");
    rig.compositor.sync();
    assert_eq!(rig.server.keys().len(), keys, "Esc 报给了 Server");
    assert!(
        !rig.compositor
            .requests
            .since(requests)
            .iter()
            .any(|request| matches!(request, Request::Key { code: KEY_ESC, .. })),
        "Esc 转给了应用"
    );
    let from = rig.server.received.len();
    rig.server
        .received
        .wait(from, "收起菜单之后的轮询", |message| {
            kind(message) == "Poll"
        });
}
