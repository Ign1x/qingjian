//! 左上皮肤装饰、半透明阴影与合成负坐标不能被当成空白裁掉。

use qingjian_render::{Layout, Mode, Rendered, Renderer, StatusCell, Theme, sample};

use super::super::Canvas;
use super::{rendered, window};
use crate::view::{Side, blit, font_library};

fn assert_transparent_crop(canvas: &Canvas, pixels: &[u8]) {
    let width = canvas.width() as usize;
    let placement = &canvas.placement;
    for (i, pixel) in pixels.chunks_exact(4).enumerate() {
        if i % width < placement.crop_x as usize || i / width < placement.crop_y as usize {
            assert_eq!(
                pixel[3],
                0,
                "裁到了非透明像素 ({}, {})",
                i % width,
                i / width
            );
        }
    }
    let mut buffer = vec![0; (placement.width * placement.height * 4) as usize];
    blit(pixels, canvas.width(), placement, &mut buffer);
    let alpha = |pixels: &[u8]| pixels.chunks_exact(4).map(|p| u64::from(p[3])).sum::<u64>();
    assert_eq!(alpha(pixels), alpha(&buffer), "贴图不能丢掉皮肤像素");
}

#[test]
fn translucent_decorations_survive_at_every_scale() {
    for scale in [1.0, 1.25, 1.5, 2.0] {
        let mut window = window(scale);
        // 卡片外的装饰，连 alpha = 1 的阴影也要保留。
        for (x, y, color) in [(2, 8, [1, 0, 0, 1]), (8, 1, [0, 40, 0, 80])] {
            let at = ((y * window.pixmap.width() + x) * 4) as usize;
            window.pixmap.data_mut()[at..at + 4].copy_from_slice(&color);
        }
        let canvas = Canvas::new(&window, None, f64::from(scale));
        assert_transparent_crop(&canvas, &canvas.compose(&window, None, None));
        let (x, y, _, _) = canvas.input_region()[0];
        assert!(x > 4 && y > 4, "装饰占的地方不接鼠标");
    }
}

#[test]
fn sakura_skin_survives_in_both_layouts() {
    let Ok(fonts) = font_library("") else {
        eprintln!("没有系统字体，跳过樱花皮肤测试");
        return;
    };
    let mut renderer = Renderer::new(fonts);
    renderer.set_reduce_motion(true);
    let theme = Theme::builtin("sakura", false).unwrap();
    for scale in [1.0, 1.25, 1.5, 2.0] {
        for layout in [Layout::Vertical, Layout::Horizontal] {
            renderer.forget();
            let window = renderer
                .render(&sample::nihao(), layout, &theme, scale)
                .unwrap();
            let canvas = Canvas::new(&window, None, f64::from(scale));
            assert_transparent_crop(&canvas, &canvas.compose(&window, None, None));
        }
        let hint = renderer.render_status(
            &[StatusCell::text("中", true)],
            &Mode::default(),
            &theme,
            scale,
        );
        {
            let hint = hint.unwrap();
            let canvas = Canvas::hint(&hint.rendered, f64::from(scale));
            assert_transparent_crop(&canvas, &canvas.compose(&hint.rendered, None, None));
        }
    }
}

fn painted(size: (u32, u32), content: (u32, u32, u32, u32), color: [u8; 4]) -> Rendered {
    let mut image = rendered(size, content, 1.0, &[]);
    for pixel in image.pixmap.data_mut().chunks_exact_mut(4) {
        pixel.copy_from_slice(&color);
    }
    image
}

#[test]
fn menu_and_keyboard_keep_their_negative_overhang() {
    let window = rendered((40, 30), (2, 2, 36, 26), 1.0, &[]);
    let menu = painted((60, 50), (20, 18, 30, 20), [80, 0, 0, 80]);
    let canvas = Canvas::new(&window, Some((&menu, (0.0, 0.0))), 1.0);
    let pixels = canvas.compose(&window, Some(&menu), None);
    assert_eq!(
        pixels.chunks_exact(4).filter(|p| p[3] == 80).count(),
        60 * 50
    );
    assert_transparent_crop(&canvas, &pixels);
    assert_eq!(canvas.anchor(20.0, 18.0), (0.0, 0.0));

    let keyboard = painted((80, 70), (20, 18, 40, 34), [0, 80, 0, 80]);
    for (flipped, side) in [
        (false, Side::Right),
        (false, Side::Away),
        (true, Side::Away),
    ] {
        let mut canvas = Canvas::new(&window, None, 1.0);
        canvas.place_key_hint(&keyboard, flipped, Some(side));
        let pixels = canvas.compose(&window, None, Some(&keyboard));
        assert_eq!(
            pixels.chunks_exact(4).filter(|p| p[3] == 80).count(),
            80 * 70
        );
        assert_transparent_crop(&canvas, &pixels);
    }
}
