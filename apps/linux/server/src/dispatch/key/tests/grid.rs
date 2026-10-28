//! 横排矩阵（`[general] horizontal_grid`）：只对握手报了 `features.grid` 的会话展开，规则与 Windows Server 一致。
use super::support::{compose, key, router};
use crate::dispatch::Router;
use qingjian_core::candidate::arrows::Focus;
use qingjian_platform::protocol::{
    ClientMessage, Frame, KeyModifiers, PROTOCOL_VERSION, SessionId,
};
use qingjian_platform::{HighlightKeys, LayoutMode};
use serde_json::{Value, json};

const TAB: u32 = 0x09;
const ESCAPE: u32 = 0x1B;
const LEFT: u32 = 0x25;
const UP: u32 = 0x26;
const RIGHT: u32 = 0x27;
const DOWN: u32 = 0x28;

fn press(router: &mut Router, code: u32) -> Frame {
    key(router, code, None, KeyModifiers::default()).2
}

fn identity() -> Value {
    json!({"generation": 1, "context": "grid", "revision": 0})
}

/// 横排、开了「横排时展开成多行」的路由器，`features_grid` 是前端握手报没报画得了矩阵；
/// `qq` 已在组句：九个短语、每页两个，五行。
fn grid_router(features_grid: bool) -> Router {
    let mut router = router(2);
    router.config.layout = LayoutMode::Horizontal;
    router.config.horizontal_grid = true;
    router.handle_linux(json!({"DisplayReporting": {
        "session": 1, "identity": identity(), "features": {"grid": features_grid}
    }}));
    compose(&mut router, "qq", KeyModifiers::default());
    router
}

/// ↓ 展开成固定视口（一行一页、行尾补空位）、← → 按阅读顺序移动、数字键选高亮所在那一行、Esc 第一下收回单行。
#[test]
fn horizontal_grid_expands_and_navigates() {
    let mut router = grid_router(true);
    // ↓：从单行展开，高亮下移一行（第二页第一个）；五行全进视口，最后一格是空位
    let frame = press(&mut router, DOWN);
    assert_eq!((frame.columns, frame.highlight, frame.page), (2, 2, 1));
    assert_eq!(frame.candidates.items.len(), 10);
    assert_eq!(frame.candidates.items[9].text, "");
    assert_eq!(frame.column_ems.len(), 2);
    let frame = press(&mut router, DOWN);
    assert_eq!((frame.highlight, frame.page), (4, 2));
    assert_eq!(press(&mut router, LEFT).highlight, 3);
    assert_eq!(press(&mut router, RIGHT).highlight, 4);
    // 高亮 4 在第三页，`2` 选第 6 个候选
    let normal = KeyModifiers::default();
    assert_eq!(
        key(&mut router, 0x32, Some('2'), normal).1.as_deref(),
        Some("第6项")
    );

    compose(&mut router, "qq", normal);
    press(&mut router, DOWN);
    let frame = press(&mut router, ESCAPE);
    assert_eq!((frame.columns, frame.highlight, frame.page), (0, 0, 1));
    assert_eq!(frame.candidates.items.len(), 2);
    assert!(press(&mut router, ESCAPE).candidates.items.is_empty());
}

/// 矩阵展开着时翻页键一次翻一屏（六行），到头夹住。
#[test]
fn horizontal_grid_page_keys_turn_screens() {
    let mut router = grid_router(true);
    press(&mut router, DOWN);
    let frame = press(&mut router, TAB);
    assert_eq!((frame.columns, frame.highlight, frame.page), (2, 8, 4));
    let shift = KeyModifiers {
        shift: true,
        ..Default::default()
    };
    let frame = key(&mut router, TAB, None, shift).2;
    assert_eq!((frame.highlight, frame.page), (0, 0));
}

/// 开关关着、或者前端没报画得了矩阵（Fcitx5 / Fcitx4 插件、IBus 前端）：横排按键与以前一样，
/// ↑ ↓ 逐个移高亮（不展开），← → 移拼音光标。
#[test]
fn horizontal_grid_off_or_undrawable_keeps_old_keys() {
    let mut off = grid_router(true);
    off.config.horizontal_grid = false;
    let mut undrawable = grid_router(false);
    for router in [&mut off, &mut undrawable] {
        let frame = press(router, DOWN);
        assert_eq!((frame.columns, frame.highlight), (0, 1));
        assert!(frame.column_ems.is_empty());
        // ← 移拼音光标：重查候选，高亮归零。帧里的光标在显示串 `q'q` 上
        let frame = press(router, LEFT);
        assert_eq!((frame.columns, frame.highlight, frame.cursor), (0, 0, 1));
    }
}

/// ← 顶在第一个候选上交还拼音光标（移动光标重查候选，矩阵收回）；交还后 ← / → 都归光标，
/// 光标回到末尾后 → 才重新归候选。
#[test]
fn horizontal_grid_walks_the_caret_and_the_cells_as_one_line() {
    let mut router = grid_router(true);
    press(&mut router, DOWN);
    press(&mut router, UP);
    let frame = press(&mut router, LEFT);
    assert_eq!((frame.columns, frame.highlight, frame.cursor), (0, 0, 1));
    let frame = press(&mut router, LEFT);
    assert_eq!((frame.highlight, frame.cursor), (0, 0));
    assert_eq!(press(&mut router, RIGHT).cursor, 1);
    assert_eq!(press(&mut router, RIGHT).cursor, 3);
    assert_eq!(press(&mut router, RIGHT).highlight, 1);
}

/// 第一排再按 ↑：高亮回到第一个候选；已在第一个上再按不动。
#[test]
fn horizontal_grid_up_at_the_first_row_returns_to_the_first_candidate() {
    let mut router = grid_router(true);
    press(&mut router, DOWN);
    assert_eq!(press(&mut router, RIGHT).highlight, 3);
    assert_eq!(press(&mut router, UP).highlight, 1);
    let frame = press(&mut router, UP);
    assert_eq!((frame.columns, frame.highlight, frame.page), (2, 0, 0));
    let frame = press(&mut router, UP);
    assert_eq!((frame.columns, frame.highlight), (2, 0));
}

/// 方向键分两区时：候选里 ↓ 展开；矩阵里 ↑ 先往上换行，到第一行再 ↑ 回拼音行，矩阵不收起（收起用 Esc）。
#[test]
fn two_area_arrows_expand_climb_and_return_to_pinyin() {
    let mut router = grid_router(true);
    router.config.highlight_keys = HighlightKeys::LeftRight;
    assert_eq!(press(&mut router, DOWN).columns, 2);
    assert_eq!(press(&mut router, UP).highlight, 0);
    let frame = press(&mut router, UP);
    assert_eq!((frame.columns, frame.highlight), (2, usize::MAX));
    assert_eq!(router.focus, Focus::Pinyin);
}

/// 矩阵展开着时前端报的格号按视口算：点视口里第 7 格上屏整份候选里的第 7 个，不是高亮那一页里的。
#[test]
fn clicks_count_cells_in_the_viewport() {
    let mut router = grid_router(true);
    press(&mut router, DOWN);
    press(&mut router, DOWN);
    let reply = router
        .handle_linux(json!({"LinuxEvent": {"session": 1, "event": {
            "Candidate": {"identity": identity(), "index": 6}
        }}}))
        .unwrap();
    assert_eq!(reply["KeyResult"]["commit"], "第7项");
}

/// 换到别的输入框再回来，展开着的矩阵跟着组句一起挂起、恢复；关掉开关后挂起的也收回。
#[test]
fn grid_is_kept_per_session_and_collapses_when_turned_off() {
    let mut router = grid_router(true);
    press(&mut router, DOWN);
    router.handle(ClientMessage::OpenSession {
        session: SessionId(2),
        app: None,
        protocol: PROTOCOL_VERSION,
    });
    router.ensure_focus(SessionId(2));
    assert!(router.grid.is_none());
    router.ensure_focus(SessionId(1));
    assert_eq!(router.current_frame().columns, 2);

    router.ensure_focus(SessionId(2));
    router.config.horizontal_grid = false;
    router.collapse_disabled_grids();
    router.ensure_focus(SessionId(1));
    assert_eq!(router.current_frame().columns, 0);
}
