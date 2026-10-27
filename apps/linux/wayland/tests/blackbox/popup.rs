//! 候选窗：非空帧画进 popup surface，有分数缩放时 viewport 设逻辑尺寸、缓冲按 1:1 像素；组句结束挂空缓冲收起。

use super::support::{Answer, KEY_N, KEY_SPACE, Picture, Request, Rig, Setup, skip};

/// 合成器要的倍数：`preferred_scale` 180，即 1.5 倍。
const SCALE: f64 = 1.5;

/// 1.5 倍下：画好的缓冲是 viewport 目标尺寸的 1.5 倍、buffer scale 保持 1；
/// 画完给 Server 回展示回执；空格上屏、帧空了就挂空缓冲收起。
#[test]
fn frame_is_drawn_into_the_popup() {
    let Some(rig) = Rig::start(Setup {
        scale: Some(180),
        ..Setup::default()
    }) else {
        return;
    };
    if !rig.frontend.painter() {
        skip("这台机器上没有界面字体，候选窗画不出来");
        return;
    }
    rig.focus();
    rig.server.answer(Answer::composing("n", &["你", "呢"]));
    rig.server.answer(Answer::commit("你"));
    let mark = rig.compositor.requests.len();
    rig.compositor.tap(KEY_N);
    // 等第一块画上东西的：主题的出现动画哪天从全透明开始，头一块就是空的
    let shown = rig
        .compositor
        .requests
        .wait(mark, "候选窗挂上画好的缓冲", |request| {
            matches!(
                request,
                Request::Popup {
                    picture: Some(Picture { painted: true, .. }),
                    ..
                }
            )
        });
    let Request::Popup {
        picture: Some(picture),
        destination,
        scale,
    } = shown
    else {
        unreachable!("等的就是挂着缓冲的 commit");
    };
    let (width, height) = destination.expect("viewport 设了目标尺寸");
    assert_eq!(scale, 1, "有 viewport 时 buffer scale 保持 1");
    assert_eq!(
        (picture.width, picture.height),
        (scaled(width), scaled(height)),
        "缓冲要正好是逻辑尺寸的 1.5 倍，合成器才按 1:1 像素贴"
    );
    rig.server.received.wait(0, "展示回执", |message| {
        message.get("DisplayAcknowledged").is_some()
    });

    let mark = rig.compositor.requests.len();
    rig.compositor.tap(KEY_SPACE);
    rig.compositor.requests.wait(mark, "候选窗收起", |request| {
        matches!(request, Request::Popup { picture: None, .. })
    });
}

fn scaled(logical: i32) -> i32 {
    (f64::from(logical) * SCALE).round() as i32
}
