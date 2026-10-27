//! 候选窗绘制的测试：位图放进缓冲的算术、像素换序、义项回报，外观设置的读取，以及要系统字体的渲染、热加载与跟随桌面的深浅。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use qingjian_core::{
    Candidate, CandidateKind, CandidateList, Language, PartOfSpeech, Sense, Translation,
};
use qingjian_platform::protocol::{Frame, PreeditKind, PreeditSegment};
use qingjian_platform::{Appearance, FontSize, GeneralConfig};
use qingjian_render::{HitRegion, HitTarget, Pixmap, Rendered, TextSizes};

use super::look::Look;
use super::{Painter, Placement, blit, renumber_senses, senses};
use crate::portal::{Desktop, Setting};

/// 每个测试自己的临时目录（先清空）。
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "qingjian-wayland-view-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 不同缩放下：左上只留 4 点阴影边，缓冲盖得住裁剩的位图，逻辑尺寸是盖得住的最小值、乘回缩放与缓冲差不到 1 像素，整数缩放正好整除。
#[test]
fn placement_keeps_four_points_and_covers_the_bitmap() {
    for scale in [1.0_f64, 1.25, 1.5, 2.0] {
        // 左上阴影边 24 × 20 点
        let (shadow_x, shadow_y) = ((24.0 * scale) as u32, (20.0 * scale) as u32);
        let keep = (4.0 * scale).round() as u32;
        for (width, height) in [(301, 207), (450, 300), (451, 451), (64, 33)] {
            let placement = Placement::new(width, height, shadow_x, shadow_y, scale);
            let what = format!("{scale} {width}×{height}: {placement:?}");
            assert_eq!(placement.crop_x, shadow_x - keep, "{what}");
            assert_eq!(placement.crop_y, shadow_y - keep, "{what}");
            let visible = [width - placement.crop_x, height - placement.crop_y];
            let buffer = [placement.width, placement.height];
            let logical = [placement.logical_width, placement.logical_height];
            for axis in 0..2 {
                let (visible, buffer, logical) = (visible[axis], buffer[axis], logical[axis]);
                assert!(buffer >= visible, "{what}");
                assert!(f64::from(logical) * scale >= f64::from(visible), "{what}");
                assert!(
                    f64::from(logical - 1) * scale < f64::from(visible),
                    "{what}"
                );
                assert!(
                    (f64::from(logical) * scale - f64::from(buffer)).abs() < 1.0,
                    "{what}"
                );
                if scale.fract() == 0.0 {
                    assert_eq!(f64::from(buffer), f64::from(logical) * scale, "{what}");
                }
            }
        }
    }
}

/// 1.5 倍下的一组实数：可见 421 → 逻辑 281 → 缓冲 round(421.5) = 422，右侧补一列透明。
#[test]
fn placement_at_one_and_a_half() {
    let placement = Placement::new(451, 300, 36, 30, 1.5);
    assert_eq!(
        placement,
        Placement {
            crop_x: 30,
            crop_y: 24,
            width: 422,
            height: 276,
            logical_width: 281,
            logical_height: 184,
        }
    );
}

/// 阴影边本来就不到 4 点时不裁；空位图、坏缩放也给出至少 1 像素的尺寸。
#[test]
fn placement_edge_cases() {
    let thin = Placement::new(100, 50, 3, 0, 1.0);
    assert_eq!((thin.crop_x, thin.crop_y), (0, 0));
    assert_eq!((thin.width, thin.height), (100, 50));
    let empty = Placement::new(0, 0, 0, 0, 1.5);
    assert_eq!((empty.width, empty.height), (2, 2));
    assert_eq!((empty.logical_width, empty.logical_height), (1, 1));
    for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let placement = Placement::new(100, 50, 24, 20, scale);
        assert_eq!((placement.crop_x, placement.crop_y), (20, 16), "{scale}");
        assert_eq!(
            (placement.width, placement.logical_width),
            (80, 80),
            "{scale}"
        );
    }
}

/// 预乘 RGBA → B,G,R,A：R、B 对调，alpha 原样；裁掉的左上不拷，右侧与下方补边是 0。
#[test]
fn blit_swizzles_crops_and_pads() {
    // 3 × 2 的位图，每个字节都不同、都不是 0
    let pixels: Vec<u8> = (1..=24).collect();
    let pixel = |x: usize, y: usize| &pixels[(y * 3 + x) * 4..][..4];
    let placement = Placement {
        crop_x: 1,
        crop_y: 1,
        width: 4,
        height: 2,
        logical_width: 4,
        logical_height: 2,
    };
    let mut dst = vec![0xAA; 4 * 2 * 4];
    blit(&pixels, 3, &placement, &mut dst);
    for x in 0..2 {
        let src = pixel(x + 1, 1);
        assert_eq!(
            dst[x * 4..][..4],
            [src[2], src[1], src[0], src[3]],
            "第 {x} 个"
        );
    }
    assert!(dst[8..].iter().all(|&byte| byte == 0), "{dst:?}");
}

#[test]
fn blit_copies_a_translucent_pixel_unchanged_but_reordered() {
    let placement = Placement::new(1, 1, 0, 0, 1.0);
    let mut dst = vec![0; 4];
    blit(&[10, 20, 30, 128], 1, &placement, &mut dst);
    assert_eq!(dst, [30, 20, 10, 128]);
}

fn rendered(targets: &[HitTarget]) -> Rendered {
    Rendered {
        pixmap: Pixmap::new(1, 1).unwrap(),
        content_x: 0,
        content_y: 0,
        content_width: 1,
        content_height: 1,
        scale: 1.0,
        next_frame: None,
        hits: targets
            .iter()
            .map(|&target| HitRegion {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
                target,
            })
            .collect(),
    }
}

#[test]
fn senses_reports_only_translations() {
    let translation = |candidate, sense| HitTarget::Translation { candidate, sense };
    let rendered = rendered(&[
        translation(0, 0),
        translation(0, 1),
        translation(3, 0),
        HitTarget::Candidate(0),
        HitTarget::Candidate(3),
        HitTarget::MenuItem(2),
    ]);
    assert_eq!(senses(&rendered), [(0, 0), (0, 1), (3, 0)]);
    assert!(senses(&self::rendered(&[HitTarget::Candidate(0)])).is_empty());
}

/// 多于 128 对 Server 会断开连接，只报前 128 对。
#[test]
fn senses_are_capped_at_128() {
    let targets: Vec<HitTarget> = (0..200)
        .map(|i| HitTarget::Translation {
            candidate: i / 2,
            sense: i % 2,
        })
        .collect();
    let reported = senses(&rendered(&targets));
    assert_eq!(reported.len(), 128);
    assert_eq!(reported[127], (63, 1));
}

#[test]
fn look_takes_the_general_section() {
    let mut general = GeneralConfig::default();
    let look = Look::new(&general);
    assert_eq!(look.theme_id, "qingjian");
    assert!(look.font.is_empty());
    assert_eq!(look.sizes, TextSizes::default());
    assert!(look.animations);

    // 旧写法：theme 写的是外观词，按内置主题
    general.theme = "dark".to_owned();
    general.font = "  LXGW WenKai ".to_owned();
    general.candidate_font_size = FontSize(18.0);
    general.annotation_font_size = FontSize(0.0);
    general.animations = false;
    assert_eq!(
        Look::new(&general),
        Look {
            theme_id: "qingjian".to_owned(),
            font: "LXGW WenKai".to_owned(),
            sizes: TextSizes {
                candidate: Some(18.0),
                annotation: None,
            },
            animations: false,
        }
    );
}

/// 没有配置文件是缺省外观，坏文件是错误（由调用方决定退回什么）。
#[test]
fn look_reads_the_config_file() {
    let dir = scratch("look");
    let path = dir.join("config.toml");
    assert_eq!(
        Look::read(&path).unwrap(),
        Look::new(&GeneralConfig::default())
    );
    std::fs::write(
        &path,
        "[general]\ntheme = \"wechat\"\nfont = \"Noto Serif CJK SC\"\n",
    )
    .unwrap();
    let look = Look::read(&path).unwrap();
    assert_eq!(
        (look.theme_id.as_str(), look.font.as_str()),
        ("wechat", "Noto Serif CJK SC")
    );
    std::fs::write(&path, "[general\n").unwrap();
    assert!(Look::read(&path).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stamp_changes_with_the_file() {
    let dir = scratch("stamp");
    let path = dir.join("config.toml");
    let missing = Look::stamp(&path);
    assert_eq!(Look::stamp(&path), missing);
    std::fs::write(&path, "a").unwrap();
    let written = Look::stamp(&path);
    assert_ne!(written, missing);
    assert_eq!(Look::stamp(&path), written);
    // 长度也算进去：同一个时间刻里改两次也看得出来
    std::fs::write(&path, "ab").unwrap();
    assert_ne!(Look::stamp(&path), written);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn look_resolves_its_theme() {
    let mut look = Look::new(&GeneralConfig::default());
    look.theme_id = "wechat".to_owned();
    assert_eq!(look.theme(None).id(), "wechat");
    look.theme_id = "no-such-theme".to_owned();
    assert_eq!(look.theme(None).id(), "qingjian");
}

/// 在用户主题目录里写一个继承青简绿的主题。
fn write_theme(dir: &Path, id: &str, accent: &str) {
    let path = dir.join(id);
    std::fs::create_dir_all(&path).unwrap();
    let json = format!(
        r##"{{ "extends": "qingjian", "meta": {{ "id": "{id}", "name": "{id}" }},
            "variables": {{ "accent": "{accent}" }} }}"##
    );
    std::fs::write(path.join("theme.json"), json).unwrap();
}

/// 热加载：戳没变不读文件；外观以外的设置变了不重画；主题变了、主题文件改了要重画；坏文件保持原样。
/// 相邻两次写的内容长度都不同，不靠修改时间的精度。
#[test]
fn reload_reports_only_changes_to_the_look() {
    let dir = scratch("reload");
    let config = dir.join("config.toml");
    let themes = dir.join("themes");
    std::fs::write(&config, "[general]\ntheme = \"wechat\"\n").unwrap();
    let Some(mut painter) = Painter::new(config.clone(), Some(themes.clone()), Arc::default())
    else {
        eprintln!("没有系统字体，跳过");
        return;
    };
    assert!(!painter.reload());
    std::fs::write(&config, "[general]\npage_size = 7\ntheme = \"wechat\"\n").unwrap();
    assert!(!painter.reload(), "只改了每页候选数");
    std::fs::write(&config, "[general]\ntheme = \"sakura\"\n").unwrap();
    assert!(painter.reload(), "换了主题");
    assert_eq!(painter.theme.id(), "sakura");
    assert!(!painter.reload());
    std::fs::write(&config, "[general\ntheme = \"wechat\"\n").unwrap();
    assert!(!painter.reload(), "坏文件");
    assert_eq!(painter.theme.id(), "sakura");
    write_theme(&themes, "peach", "#ff8800");
    assert!(painter.reload(), "主题目录变了");
    std::fs::write(
        &config,
        "[general]\ntheme = \"peach\"\nanimations = false\n",
    )
    .unwrap();
    assert!(painter.reload(), "换到用户主题");
    assert_eq!(painter.theme.id(), "peach");
    write_theme(&themes, "peach", "#0088ff");
    assert!(painter.reload(), "在用的主题文件改了");
    let _ = std::fs::remove_dir_all(&dir);
}

/// 一个候选：`reading` 是 `display` 拼在注记最前面的读音（emoji 跟着的词、拆字的拼音），`senses` 是各条译词与是不是生词。
fn candidate(text: &str, reading: Option<&str>, senses: &[(&str, bool)]) -> Candidate {
    Candidate {
        text: text.to_owned(),
        kind: CandidateKind::Chinese,
        syllables: Vec::new(),
        reading: reading.map(str::to_owned),
        translation: (!senses.is_empty()).then(|| {
            Translation::new(
                Language::English,
                senses
                    .iter()
                    .map(|&(text, fresh)| Sense {
                        part_of_speech: Some(PartOfSpeech::Interjection),
                        text: text.to_owned(),
                        reading: None,
                        fresh,
                    })
                    .collect(),
            )
        }),
        aux_code: None,
    }
}

/// 敲 nihao 时 Server 发来的帧（与渲染器的样例帧相仿），外加一个问字那样读音、译词都有的候选。
fn nihao() -> Frame {
    Frame {
        preedit: vec![PreeditSegment {
            text: "ni'hao".to_owned(),
            kind: PreeditKind::Typed,
        }],
        cursor: 6,
        candidates: CandidateList {
            items: vec![
                candidate("你好", None, &[("hello", false), ("hi", true)]),
                candidate("👋", Some("你好"), &[]),
                candidate("你好像", None, &[("you seem", true)]),
                candidate("鑫", Some("xīn"), &[("prosperous", false)]),
            ],
        },
        page_count: 6,
        translation_pos: true,
        ..Frame::default()
    }
}

/// 渲染器的译词编号换成 Core 的：读音占的那一号去掉、后面的往前挪，没有译文的、越界的、没有这个候选的都不报。
#[test]
fn senses_are_renumbered_to_core() {
    let translation = |candidate, sense| HitTarget::Translation { candidate, sense };
    let mut hits = rendered(&[
        translation(0, 0),
        translation(0, 1),
        translation(1, 0),
        translation(2, 0),
        translation(2, 1),
        translation(3, 0),
        translation(3, 1),
        translation(9, 0),
        HitTarget::Candidate(1),
    ])
    .hits;
    renumber_senses(&nihao(), &mut hits);
    let targets: Vec<HitTarget> = hits.iter().map(|region| region.target).collect();
    assert_eq!(
        targets,
        [
            translation(0, 0),
            translation(0, 1),
            translation(2, 0),
            translation(3, 0),
            HitTarget::Candidate(1),
        ]
    );
}

/// 协议帧画得出来，回报的义项过得了 Server 的校验（读音不算译词）；外观深色与浅色不一样，
/// 「跟随系统」看桌面：没有偏好时与浅色一样，偏好深色时与深色一样。
#[test]
fn renders_a_protocol_frame() {
    let dir = scratch("render");
    let config = dir.join("config.toml");
    // 关掉动画：同一帧画两次结果一样，才好比较像素
    std::fs::write(&config, "[general]\nanimations = false\n").unwrap();
    let desktop = Arc::new(Desktop::default());
    let Some(mut painter) = Painter::new(config, None, Arc::clone(&desktop)) else {
        eprintln!("没有系统字体，跳过");
        return;
    };
    let mut frame = nihao();
    frame.appearance = Appearance::Light;
    let light = painter.render(&frame, 1.5).unwrap();
    assert!(light.content_x > 0 && light.content_width > 0);
    assert_eq!(light.scale, 1.5);
    let reported = senses(&light);
    assert!(
        reported.contains(&(0, 0)) && reported.contains(&(3, 0)),
        "{reported:?}"
    );
    // 与 Server 一样校验：有这个候选、它有译文、义项不越界
    assert!(
        reported.iter().all(|&(candidate, sense)| {
            frame.candidates.items[candidate]
                .translation
                .as_ref()
                .is_some_and(|translation| sense < translation.senses().len())
        }),
        "{reported:?}"
    );

    painter.forget();
    frame.appearance = Appearance::Dark;
    let dark = painter.render(&frame, 1.5).unwrap();
    assert_ne!(dark.pixmap.data(), light.pixmap.data());

    painter.forget();
    frame.appearance = Appearance::System;
    let system = painter.render(&frame, 1.5).unwrap();
    assert_eq!(system.pixmap.data(), light.pixmap.data());

    desktop.apply(Setting::ColorScheme { dark: true });
    painter.forget();
    let system = painter.render(&frame, 1.5).unwrap();
    assert_eq!(system.pixmap.data(), dark.pixmap.data());
    assert!(painter.tick().is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// 桌面换了深浅：画着「跟随系统」的帧时下一次 reload 要重画（只报一次），画着固定深浅的不用；
/// 减少动态效果的开关变了也重画。
#[test]
fn reload_follows_the_desktop() {
    let dir = scratch("desktop");
    let config = dir.join("config.toml");
    std::fs::write(&config, "[general]\nanimations = false\n").unwrap();
    let desktop = Arc::new(Desktop::default());
    let Some(mut painter) = Painter::new(config, None, Arc::clone(&desktop)) else {
        eprintln!("没有系统字体，跳过");
        return;
    };
    let mut frame = nihao();
    frame.appearance = Appearance::System;
    painter.render(&frame, 1.0).unwrap();
    assert!(!painter.reload());
    desktop.apply(Setting::ColorScheme { dark: true });
    assert!(painter.reload(), "跟随系统的帧换成深色");
    assert!(!painter.reload(), "同一次变化只报一次");

    frame.appearance = Appearance::Light;
    painter.render(&frame, 1.0).unwrap();
    desktop.apply(Setting::ColorScheme { dark: false });
    assert!(!painter.reload(), "浅色的帧不跟着桌面");

    desktop.apply(Setting::Animations(false));
    assert!(painter.reload(), "减少动态效果开了");
    assert!(!painter.reload());
    let _ = std::fs::remove_dir_all(&dir);
}
