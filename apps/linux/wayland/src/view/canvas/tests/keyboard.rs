//! 键盘摆进合成位图：在候选窗右边并排，或翻到上方时候选窗下移让位；键盘从来不收鼠标。

use qingjian_render::Rendered;

use super::super::Canvas;
use super::{rendered, window};
use crate::session::Spot;
use crate::view::Side;

/// 一块键盘：内容区 120 × 80，左上阴影 10 × 8（位图 140 × 96）。
fn keyboard() -> Rendered {
    rendered((140, 96), (10, 8, 120, 80), 1.0, &[])
}

/// 未翻转、这段输入用的是右边：键盘并排在候选窗右边，合成位图变宽，裁边仍按候选窗的阴影，候选照旧点得中，键盘不占收鼠标的区域。
/// （先试哪一侧由 `key_hint::place` 的测试管：这块候选窗宽是高的两倍，不指定会先往下。）
#[test]
fn the_keyboard_sits_beside_the_window_on_the_right() {
    let mut window = window(1.0);
    window.pixmap.data_mut().fill(100);
    let mut board = keyboard();
    for pixel in board.pixmap.data_mut().chunks_exact_mut(4) {
        pixel.copy_from_slice(&[255, 0, 0, 255]);
    }
    let mut canvas = Canvas::new(&window, None, 1.0);
    assert_eq!(
        canvas.place_key_hint(&board, false, Some(Side::Right)),
        Side::Right
    );
    assert_eq!(canvas.width(), 314, "候选窗 200 像素，键盘伸到 174 + 140");
    assert_eq!(
        (canvas.placement.crop_x, canvas.placement.crop_y),
        (16, 12),
        "裁边仍按候选窗的阴影"
    );
    assert_eq!(canvas.spot(20.0, 40.0), Some(Spot::Candidate(0)));
    assert_eq!(canvas.input_region(), [(4, 4, 160, 80)], "键盘不收鼠标");

    let pixels = canvas.compose(&window, None, Some(&board));
    let at = |x: usize, y: usize| &pixels[(y * 314 + x) * 4..(y * 314 + x) * 4 + 4];
    assert_eq!(
        at(250, 8),
        [255, 0, 0, 255],
        "键盘摆在候选窗右边（位图原点 = 内容区左上角减阴影）"
    );
    assert_eq!(at(0, 0), [100, 100, 100, 100], "候选窗还在原地");
}

/// 翻转（候选窗在光标上方）：键盘摆在上面，候选窗与收鼠标的区域一起下移，总高变大；键盘那几行不在收鼠标的区域里。
#[test]
fn a_flipped_keyboard_pushes_the_window_down() {
    let window = window(1.0);
    let plain = Canvas::new(&window, None, 1.0);
    let board = keyboard();
    let mut canvas = Canvas::new(&window, None, 1.0);
    assert_eq!(canvas.place_key_hint(&board, true, None), Side::Away);
    assert_eq!(canvas.width(), 200);
    assert!(
        canvas.placement.logical_height > plain.placement.logical_height,
        "键盘与候选窗叠起来，总高变大"
    );
    assert_eq!(
        canvas.placement.logical_height, 192,
        "键盘 96 + 间隙 + 候选窗下移"
    );
    assert_eq!(canvas.placement.crop_y, 4, "键盘的上阴影也只留 4 点");
    assert_eq!(
        canvas.input_region(),
        [(4, 88, 160, 80)],
        "候选窗与收鼠标的区域一起下移"
    );
    assert_eq!(
        canvas.spot(20.0, 120.0),
        Some(Spot::Candidate(0)),
        "候选仍点得中"
    );
}
