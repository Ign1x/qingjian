//! 键位提示：握手报 `key_hint`；回包带 `key_hint` 时键盘画在候选窗旁边——合成器没翻（光标在候选窗上方）时
//! 放在右边、窗口变宽，翻了改放上方、窗口变高；键盘那块不收鼠标；下一帧不带提示（组句结束）就收起。

use qingjian_platform::protocol::linux::{KeyHintContent, KeyHintView, RootChartId};

use super::support::{Answer, KEY_N, KEY_SPACE, Picture, Request, Rig, Setup, skip};

/// 候选窗宽度的容差（逻辑像素）：收起键盘后窗口该回到第一帧的宽度。
const TOLERANCE: i32 = 2;

/// 键位提示：86 版五笔字根图，不透明、原大。
fn hint() -> KeyHintView {
    KeyHintView {
        content: KeyHintContent::Roots(RootChartId::Wubi86),
        opacity: 100,
        size: 100,
    }
}

/// 握手报自己能画横排矩阵与键位提示（Server 据此才发这两样）。
#[test]
fn handshake_reports_grid_and_key_hint() {
    let Some(rig) = Rig::start(Setup::default()) else {
        return;
    };
    // Server 是懒连的：点进输入框（activate）才会连上去握手
    rig.focus();
    let hello = rig.server.received.wait(0, "LinuxHello", |message| {
        message.get("LinuxHello").is_some()
    });
    let features = &hello["LinuxHello"]["features"];
    assert_eq!(features["grid"].as_bool(), Some(true), "{hello}");
    assert_eq!(features["key_hint"].as_bool(), Some(true), "{hello}");
}

/// 带键位提示的帧：键盘先放候选窗右边（窗口变宽、高度取两者较大、收鼠标的区域不变）；
/// 合成器报光标在候选窗上方（y ≥ 0）之后改放上方（高度加上键盘的高）；报回去又回到右边；
/// 组句结束的下一帧不带提示，键盘收起。
#[test]
fn the_keyboard_follows_the_compositor_flip() {
    let Some(rig) = Rig::start(Setup::default()) else {
        return;
    };
    if !rig.frontend.painter() {
        skip("这台机器上没有界面字体，候选窗画不出来");
        return;
    }
    rig.focus();

    // 第一帧不带键位提示：记下候选窗的尺寸与收鼠标的区域
    rig.server.answer(Answer::composing("n", &["你", "呢"]));
    let mark = rig.compositor.requests.len();
    rig.compositor.tap(KEY_N);
    let (w0, h0) = painted(&rig, mark, "没有键位提示的候选窗");
    let region = rig.compositor.input_region();

    // 同样的候选 + 键位提示：键盘放在候选窗右边，窗口变宽
    rig.server
        .answer(Answer::composing("n", &["你", "呢"]).with_key_hint(hint()));
    let mark = rig.compositor.requests.len();
    rig.compositor.tap(KEY_N);
    let right = wider_than(&rig, mark, w0);
    assert!(
        right.1 >= h0,
        "高度取候选窗与键盘中较大的那个：{right:?}，候选窗 {h0}"
    );
    assert_eq!(
        rig.compositor.input_region(),
        region,
        "键盘那块不收鼠标，收鼠标的区域仍只有候选窗卡片"
    );

    // 合成器报光标在候选窗上方（y ≥ 0）：键盘改放候选窗上方，窗口变高
    let mark = rig.compositor.requests.len();
    rig.compositor.text_input_rectangle(0, 400, 2, 20);
    let flipped = taller_than(&rig, mark, right.1);
    assert!(
        flipped.1 > h0,
        "翻到上方之后高度要大于候选窗本身：{flipped:?}，候选窗 {h0}"
    );

    // 报回去（y < 0，不翻）：回到键盘在右边时的尺寸
    let mark = rig.compositor.requests.len();
    rig.compositor.text_input_rectangle(0, -20, 2, 20);
    let back = rig
        .compositor
        .requests
        .wait(mark, "键盘回到右边", |request| {
            matches!(
                request,
                Request::Popup {
                    picture: Some(Picture { painted: true, .. }),
                    destination: Some((width, height)),
                    ..
                } if (*width, *height) == right
            )
        });
    let Request::Popup {
        destination: Some(back),
        ..
    } = back
    else {
        unreachable!("等的就是挂着缓冲、设了目标尺寸的 commit");
    };
    assert!(back.1 <= flipped.1, "回到右边之后不该比翻到上方时还高");

    // 上屏：组句结束的空帧不带键位提示，候选窗收起
    rig.server.answer(Answer::commit("你"));
    let mark = rig.compositor.requests.len();
    rig.compositor.tap(KEY_SPACE);
    rig.compositor.requests.wait(mark, "候选窗收起", |request| {
        matches!(request, Request::Popup { picture: None, .. })
    });

    // 重新组句，这一帧不带键位提示：键盘不再出现，宽度回到第一帧
    rig.server.answer(Answer::composing("n", &["你", "呢"]));
    let mark = rig.compositor.requests.len();
    rig.compositor.tap(KEY_N);
    let (width, _) = painted(&rig, mark, "收起键盘的候选窗");
    assert!(
        (width - w0).abs() <= TOLERANCE,
        "不带键位提示时宽度要回到 {w0}，实际 {width}"
    );
}

/// 从 `from` 起等一块画上东西的候选窗 commit，交出它的目标尺寸。
fn painted(rig: &Rig, from: usize, what: &str) -> (i32, i32) {
    let request = rig.compositor.requests.wait(from, what, |request| {
        matches!(
            request,
            Request::Popup {
                picture: Some(Picture { painted: true, .. }),
                destination: Some(_),
                ..
            }
        )
    });
    destination(&request)
}

/// 从 `from` 起等一块宽度大于 `width` 的画好的候选窗 commit。
fn wider_than(rig: &Rig, from: usize, width: i32) -> (i32, i32) {
    let request = rig
        .compositor
        .requests
        .wait(from, "键盘在右边的候选窗", |request| {
            matches!(
                request,
                Request::Popup {
                    picture: Some(Picture { painted: true, .. }),
                    destination: Some((wider, _)),
                    ..
                } if *wider > width
            )
        });
    destination(&request)
}

/// 从 `from` 起等一块高度大于 `height` 的画好的候选窗 commit。
fn taller_than(rig: &Rig, from: usize, height: i32) -> (i32, i32) {
    let request =
        rig.compositor
            .requests
            .wait(from, "键盘在候选窗上方的候选窗", |request| {
                matches!(
                    request,
                    Request::Popup {
                        picture: Some(Picture { painted: true, .. }),
                        destination: Some((_, taller)),
                        ..
                    } if *taller > height
                )
            });
    destination(&request)
}

fn destination(request: &Request) -> (i32, i32) {
    let Request::Popup {
        destination: Some(destination),
        ..
    } = request
    else {
        unreachable!("等的就是设了目标尺寸的 commit：{request:?}");
    };
    *destination
}
