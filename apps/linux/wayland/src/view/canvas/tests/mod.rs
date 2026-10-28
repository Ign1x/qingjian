//! 合成位图的算术：surface 坐标认落点（裁边、缩放、菜单盖在上面）、收鼠标的区域、菜单的位置与叠图。

mod keyboard;

use qingjian_render::{HitRegion, HitTarget, Pixmap, Rendered};

use super::Canvas;
use crate::session::Spot;
use crate::view::paste;

/// 一块点击区域：目标与它在内容区里的像素矩形 `(x, y, 宽, 高)`。
pub(super) type Hit = (HitTarget, (f32, f32, f32, f32));

/// 一张假的渲染结果：位图大小、内容区 `(x, y, 宽, 高)`、倍数与点击区域（内容区像素）。
pub(super) fn rendered(
    size: (u32, u32),
    content: (u32, u32, u32, u32),
    scale: f32,
    hits: &[Hit],
) -> Rendered {
    Rendered {
        pixmap: Pixmap::new(size.0, size.1).unwrap(),
        content_x: content.0,
        content_y: content.1,
        content_width: content.2,
        content_height: content.3,
        scale,
        next_frame: None,
        hits: hits
            .iter()
            .map(|&(target, (x, y, width, height))| HitRegion {
                x,
                y,
                width,
                height,
                target,
            })
            .collect(),
    }
}

/// 两个候选的竖排候选窗，第一个候选右侧有一条译词（译词排在前面，先命中）。`scale` 倍，各尺寸按点给。
pub(super) fn window(scale: f32) -> Rendered {
    let px = |points: f32| points * scale;
    let size = |points: u32| (points as f32 * scale) as u32;
    rendered(
        (size(200), size(120)),
        (size(20), size(16), size(160), size(80)),
        scale,
        &[
            (
                HitTarget::Translation {
                    candidate: 0,
                    sense: 0,
                },
                (px(70.0), px(30.0), px(40.0), px(20.0)),
            ),
            (
                HitTarget::Candidate(0),
                (px(8.0), px(30.0), px(100.0), px(20.0)),
            ),
            (
                HitTarget::Candidate(1),
                (px(8.0), px(52.0), px(100.0), px(20.0)),
            ),
        ],
    )
}

/// 两项的菜单，左上阴影 12 × 10。
pub(super) fn menu() -> Rendered {
    rendered(
        (140, 100),
        (12, 10, 116, 80),
        1.0,
        &[
            (HitTarget::MenuItem(0), (4.0, 4.0, 108.0, 24.0)),
            (HitTarget::MenuItem(1), (4.0, 28.0, 108.0, 24.0)),
        ],
    )
}

/// 1 倍：左上阴影裁到 4 点，surface 坐标加回裁掉的边就是位图坐标；译词算它那个候选，卡片上别处与卡片外都不算。
#[test]
fn spots_follow_the_hits_through_the_crop() {
    let canvas = Canvas::new(&window(1.0), None, 1.0);
    assert_eq!((canvas.placement.crop_x, canvas.placement.crop_y), (16, 12));
    assert_eq!(canvas.spot(20.0, 40.0), Some(Spot::Candidate(0)));
    assert_eq!(canvas.spot(90.0, 40.0), Some(Spot::Candidate(0)), "译词");
    assert_eq!(canvas.spot(20.0, 60.0), Some(Spot::Candidate(1)));
    assert_eq!(canvas.spot(150.0, 10.0), None, "卡片上没有候选的地方");
    assert_eq!(canvas.spot(2.0, 2.0), None, "左上留下的阴影");
    assert_eq!(canvas.spot(175.0, 100.0), None, "右下的阴影");
    assert_eq!(canvas.input_region(), [(4, 4, 160, 80)]);
    assert_eq!(canvas.describe(), "candidate0@62,44 candidate1@62,66");
    assert_eq!(canvas.anchor(20.0, 40.0), (16.0, 36.0));
}

/// 1.5 倍：缓冲比逻辑尺寸大 1.5 倍，surface 坐标先乘回像素；同一个候选落在与 1 倍时同一处，收鼠标的区域也一样。
#[test]
fn spots_follow_the_hits_at_one_and_a_half() {
    let canvas = Canvas::new(&window(1.5), None, 1.5);
    assert_eq!((canvas.placement.crop_x, canvas.placement.crop_y), (24, 18));
    assert_eq!(
        (
            canvas.placement.logical_width,
            canvas.placement.logical_height
        ),
        (184, 108)
    );
    assert_eq!(canvas.spot(20.0, 40.0), Some(Spot::Candidate(0)));
    assert_eq!(canvas.spot(20.0, 60.0), Some(Spot::Candidate(1)));
    assert_eq!(canvas.input_region(), [(4, 4, 160, 80)]);
    assert_eq!(canvas.describe(), "candidate0@62,44 candidate1@62,66");
    let (x, y) = canvas.anchor(20.0, 40.0);
    assert!(
        (x - 16.0).abs() < 0.01 && (y - 36.0).abs() < 0.01,
        "{x}, {y}"
    );
}

/// 提示与候选沿用同一套裁边、缩放；即使渲染结果带命中区域，提示也不收鼠标。
#[test]
fn the_hint_has_no_pointer_targets_or_input_region() {
    let rendered = window(1.5);
    let hint = Canvas::hint(&rendered, 1.5);
    let candidate = Canvas::new(&rendered, None, 1.5);
    assert_eq!(hint.placement, candidate.placement);
    assert_eq!(hint.spot(20.0, 40.0), None);
    assert!(hint.input_region().is_empty());
    assert!(hint.describe().is_empty());
    assert_eq!(
        hint.compose(&rendered, None, None).as_ref(),
        rendered.pixmap.data()
    );
}

/// 菜单的卡片左上角摆在右键按下的地方，盖在候选窗上面、先认它；伸出候选窗的部分让合成位图变大，裁边不变，候选窗不挪。
#[test]
fn the_menu_lies_over_the_window() {
    let window = window(1.0);
    let menu = menu();
    let canvas = Canvas::new(&window, Some((&menu, (16.0, 36.0))), 1.0);
    assert_eq!(canvas.width(), 200);
    assert_eq!(
        (canvas.placement.crop_x, canvas.placement.crop_y),
        (16, 12),
        "按候选窗的阴影裁"
    );
    assert_eq!(
        (
            canvas.placement.logical_width,
            canvas.placement.logical_height
        ),
        (184, 130),
        "菜单伸到候选窗下面，popup 跟着变高"
    );
    assert_eq!(
        canvas.spot(30.0, 50.0),
        Some(Spot::MenuItem(0)),
        "盖住了候选"
    );
    assert_eq!(canvas.spot(30.0, 70.0), Some(Spot::MenuItem(1)));
    assert_eq!(canvas.spot(22.0, 42.0), Some(Spot::Menu), "菜单的边");
    assert_eq!(canvas.spot(150.0, 30.0), None, "菜单外、候选窗卡片上");
    assert_eq!(canvas.spot(20.0, 20.0), None);
    assert_eq!(
        canvas.spot(15.0, 60.0),
        Some(Spot::Candidate(1)),
        "菜单左边露着的候选"
    );
    assert_eq!(canvas.input_region(), [(4, 4, 160, 80), (20, 40, 116, 80)]);
    assert_eq!(
        canvas.describe(),
        "candidate0@62,44 candidate1@62,66 item0@78,56 item1@78,80"
    );
}

/// 合成：先铺候选窗、再叠菜单（摆在锚点处）；菜单伸出右下时合成位图跟着变大；没有菜单时直接用候选窗的位图。
#[test]
fn compose_lays_the_menu_over_the_window() {
    let mut window = rendered((6, 4), (1, 1, 4, 2), 1.0, &[]);
    window.pixmap.data_mut().fill(100);
    let mut menu = rendered((2, 2), (0, 0, 2, 2), 1.0, &[]);
    for pixel in menu.pixmap.data_mut().chunks_exact_mut(4) {
        pixel.copy_from_slice(&[255, 0, 0, 255]);
    }

    let canvas = Canvas::new(&window, None, 1.0);
    assert!(matches!(
        canvas.compose(&window, None, None),
        std::borrow::Cow::Borrowed(_)
    ));

    let canvas = Canvas::new(&window, Some((&menu, (2.0, 1.0))), 1.0);
    assert_eq!(canvas.width(), 6);
    let pixels = canvas.compose(&window, Some(&menu), None);
    let at = |x: usize, y: usize| &pixels[(y * 6 + x) * 4..(y * 6 + x) * 4 + 4];
    assert_eq!(at(3, 2), [255, 0, 0, 255]);
    assert_eq!(at(4, 3), [255, 0, 0, 255]);
    assert_eq!(at(2, 2), [100, 100, 100, 100]);

    let canvas = Canvas::new(&window, Some((&menu, (4.0, 2.0))), 1.0);
    assert_eq!(canvas.width(), 7);
    let pixels = canvas.compose(&window, Some(&menu), None);
    assert_eq!(pixels.len(), 7 * 5 * 4);
    assert_eq!(
        &pixels[(4 * 7 + 6) * 4..(4 * 7 + 6) * 4 + 4],
        [255, 0, 0, 255]
    );
    assert_eq!(
        &pixels[(4 * 7) * 4..(4 * 7) * 4 + 4],
        [0, 0, 0, 0],
        "候选窗之外补透明"
    );
}

/// 叠图按预乘的 source-over：不透明的盖住、透明的不动、半透明的按 alpha 混；出了界的部分不画，负的位置也行。
#[test]
fn paste_blends_and_clips() {
    let mut dst: Vec<u8> = [100, 100, 100, 255].repeat(3 * 2);
    let src = [
        [255, 0, 0, 255],
        [0, 0, 0, 0],
        [0, 64, 0, 128],
        [0, 0, 0, 0],
    ]
    .concat();
    paste(&src, 2, (1, 0), &mut dst, 3);
    let at = |dst: &[u8], x: usize, y: usize| dst[(y * 3 + x) * 4..(y * 3 + x) * 4 + 4].to_vec();
    assert_eq!(at(&dst, 1, 0), [255, 0, 0, 255]);
    assert_eq!(at(&dst, 2, 0), [100, 100, 100, 255]);
    assert_eq!(at(&dst, 1, 1), [50, 114, 50, 255]);
    assert_eq!(at(&dst, 0, 0), [100, 100, 100, 255]);

    let before = dst.clone();
    paste(&src, 2, (-1, -1), &mut dst, 3);
    assert_eq!(dst, before, "只有透明的那一角落在里面");
    paste(&src, 2, (2, 1), &mut dst, 3);
    assert_eq!(at(&dst, 2, 1), [255, 0, 0, 255]);
    paste(&src, 2, (5, 5), &mut dst, 3);
    paste(&src, 2, (-5, 0), &mut dst, 3);
}
