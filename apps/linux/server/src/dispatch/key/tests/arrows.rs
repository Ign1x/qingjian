//! 组句中的方向键：高亮移动键（`[shortcut] highlight_keys`）与横排时方向键分拼音行、候选两区。
use super::support::{compose, key, router};
use crate::dispatch::Router;
use qingjian_core::candidate::arrows::Focus;
use qingjian_platform::protocol::{
    ClientMessage, Frame, KeyModifiers, KeyOutcome, PROTOCOL_VERSION, SessionId,
};
use qingjian_platform::{HighlightKeys, LayoutMode};

const HOME: u32 = 0x24;
const LEFT: u32 = 0x25;
const UP: u32 = 0x26;
const RIGHT: u32 = 0x27;
const DOWN: u32 = 0x28;

fn press(router: &mut Router, code: u32) -> (KeyOutcome, Option<String>, Frame) {
    key(router, code, None, KeyModifiers::default())
}

/// `qq` 组句中（九个自定义短语）、高亮移动键选 `←` / `→` 的路由器。
fn leftright(layout: LayoutMode) -> Router {
    let mut router = router(2);
    router.config.highlight_keys = HighlightKeys::LeftRight;
    router.config.layout = layout;
    compose(&mut router, "qq", KeyModifiers::default());
    router
}

#[test]
fn default_keeps_up_down_on_the_highlight_and_left_right_on_the_caret() {
    let mut router = router(5);
    compose(&mut router, "qq", KeyModifiers::default());
    let end = router.current_frame().cursor;
    assert_eq!(press(&mut router, DOWN).2.highlight, 1);
    assert_eq!(press(&mut router, UP).2.highlight, 0);
    let (outcome, _, frame) = press(&mut router, LEFT);
    assert_eq!((outcome, frame.highlight), (KeyOutcome::Consumed, 0));
    assert!(
        frame.cursor < end,
        "缺省下 ← 走拼音光标：{end} → {}",
        frame.cursor
    );
}

/// 竖排选 `←` / `→`：拼音行与候选连成一条，`→` 到光标末位溢出成选词，`←` 退到首个候选再回光标。
#[test]
fn leftright_walks_the_caret_and_the_candidates_as_one_line() {
    let mut router = leftright(LayoutMode::Vertical);
    let end = router.current_frame().cursor;
    let frame = press(&mut router, RIGHT).2;
    assert_eq!((frame.highlight, frame.cursor), (1, end));
    let frame = press(&mut router, LEFT).2;
    assert_eq!((frame.highlight, frame.cursor), (0, end));
    let middle = press(&mut router, LEFT).2;
    assert!(middle.cursor < end, "高亮在首位时 ← 回拼音光标");
    assert_eq!(middle.highlight, 0);
    // 光标还没回到末位：→ 走光标，不是选词
    let frame = press(&mut router, RIGHT).2;
    assert!(frame.cursor > middle.cursor);
    assert_eq!(frame.highlight, 0);
}

/// 选 `←` / `→` 时 `↑` / `↓` 吃掉不动；高亮与光标都到头时 `←` 也吃掉，漏给应用会挪插入符。
#[test]
fn leftright_swallows_up_down_and_left_at_both_ends() {
    let mut router = leftright(LayoutMode::Vertical);
    for code in [DOWN, UP] {
        let (outcome, commit, frame) = press(&mut router, code);
        assert_eq!(
            (outcome, commit, frame.highlight),
            (KeyOutcome::Consumed, None, 0)
        );
    }
    assert_eq!(router.engine.composition().text(), "qq");
    press(&mut router, HOME);
    let (outcome, _, frame) = press(&mut router, LEFT);
    assert_eq!(
        (outcome, frame.cursor, frame.highlight),
        (KeyOutcome::Consumed, 0, 0)
    );
}

#[test]
fn arrows_pass_through_when_not_composing() {
    let mut router = router(5);
    router.config.highlight_keys = HighlightKeys::LeftRight;
    for layout in [LayoutMode::Vertical, LayoutMode::Horizontal] {
        router.config.layout = layout;
        for code in [LEFT, RIGHT, UP, DOWN] {
            assert_eq!(press(&mut router, code).0, KeyOutcome::Passthrough);
        }
    }
}

/// 横排选 `←` / `→`：候选里 ↑ 进拼音行（候选不画高亮），← → 只移拼音光标（到末尾不再溢出成选词），↓ 回候选。
/// 帧里的光标在显示串上——单声母 `q` 后自动补的 `'` 让末尾是 3。
#[test]
fn two_area_arrows_switch_between_pinyin_and_candidates() {
    let mut router = leftright(LayoutMode::Horizontal);
    let frame = press(&mut router, UP).2;
    assert_eq!((frame.highlight, frame.cursor), (usize::MAX, 3));
    assert_eq!(router.focus, Focus::Pinyin);
    assert_eq!(press(&mut router, LEFT).2.cursor, 1);
    assert_eq!(press(&mut router, RIGHT).2.cursor, 3);
    let frame = press(&mut router, RIGHT).2;
    assert_eq!((frame.highlight, frame.cursor), (usize::MAX, 3));
    assert_eq!(press(&mut router, DOWN).2.highlight, 0);
    assert_eq!(press(&mut router, RIGHT).2.highlight, 1);
    // 没开「横排时展开成多行」：候选里 ↓ 不动
    let frame = press(&mut router, DOWN).2;
    assert_eq!((frame.highlight, frame.columns), (1, 0));
}

/// 拼音行里按空格照样上屏原来高亮的那个；缓冲清空后新一轮从候选开始。
#[test]
fn two_area_focus_returns_to_candidates_after_the_buffer_empties() {
    let mut router = leftright(LayoutMode::Horizontal);
    press(&mut router, UP);
    let committed = key(&mut router, 0x20, Some(' '), KeyModifiers::default()).1;
    assert_eq!(committed.as_deref(), Some("第1项"));
    compose(&mut router, "qq", KeyModifiers::default());
    assert_eq!(router.current_frame().highlight, 0);

    press(&mut router, UP);
    key(&mut router, 0x1B, None, KeyModifiers::default());
    assert_eq!(router.focus, Focus::Candidates, "Esc 清空缓冲也回到候选");
}

/// 换到别的输入框再回来：拼音行里的焦点跟着这个输入框的组句一起挂起、恢复。
#[test]
fn two_area_focus_is_kept_per_session() {
    let mut router = leftright(LayoutMode::Horizontal);
    press(&mut router, UP);
    router.handle(ClientMessage::OpenSession {
        session: SessionId(2),
        app: None,
        protocol: PROTOCOL_VERSION,
    });
    router.ensure_focus(SessionId(2));
    assert_eq!(router.focus, Focus::Candidates);
    router.ensure_focus(SessionId(1));
    assert_eq!(router.focus, Focus::Pinyin);
    assert_eq!(router.current_frame().highlight, usize::MAX);
}
