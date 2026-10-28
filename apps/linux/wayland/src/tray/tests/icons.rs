//! 托盘图标：像素换成 StatusNotifierItem 要的样子，三种状态各有全部尺寸。

use bamboo_slip::Pixmap;
use bamboo_slip::tiny_skia::PremultipliedColorU8;
use qingjian_platform::protocol::linux::InputMode;

use super::super::icons::{Icons, picture};

/// 预乘 RGBA → 不预乘的 A、R、G、B：半透明的纯红是 (128, 255, 0, 0)，全透明是四个 0。
#[test]
fn pixels_become_straight_argb() {
    let mut pixmap = Pixmap::new(2, 1).expect("位图");
    pixmap.pixels_mut()[0] = PremultipliedColorU8::from_rgba(128, 0, 0, 128).expect("颜色");
    let (width, height, bytes) = picture(&pixmap);
    assert_eq!((width, height), (2, 1));
    assert_eq!(bytes, [128, 255, 0, 0, 0, 0, 0, 0]);
}

/// 不知道模式、中、英三种各有 16–64 六种尺寸；中与英画的不一样，中 / 英是不透明的方块、四角圆掉（透明）。
#[test]
fn every_state_has_every_size() {
    let icons = Icons::new();
    let chinese = InputMode::default();
    let english = InputMode {
        english: true,
        ..InputMode::default()
    };
    for mode in [None, Some(&chinese), Some(&english)] {
        let sizes: Vec<_> = icons
            .get(mode)
            .iter()
            .map(|(width, height, bytes)| {
                assert_eq!(bytes.len(), (width * height * 4) as usize);
                (*width, *height)
            })
            .collect();
        assert_eq!(
            sizes,
            [(16, 16), (22, 22), (24, 24), (32, 32), (48, 48), (64, 64)],
            "{mode:?}"
        );
    }
    assert_ne!(icons.get(Some(&chinese)), icons.get(Some(&english)));
    for (width, _, bytes) in icons.get(Some(&chinese)) {
        let alpha = |x: i32, y: i32| bytes[((y * width + x) * 4) as usize];
        assert!(alpha(0, 0) < 64, "{width} 的左上角在圆角外");
        assert_eq!(alpha(width / 2, 1), 255, "{width} 的上沿正中是方块");
    }
}
