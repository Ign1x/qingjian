//! 横排矩阵：帧上给了每行几格（`columns`）时，9 个候选按行优先排成 3 × 3；
//! 可点区域的名字是视口里的格号（`candidate0`…`candidate8`），点第几格就报第几格。

use qingjian_platform::LayoutMode;

use super::support::{Answer, BUTTON_LEFT, KEY_N, Rig, Setup, kind, skip};

/// 九宫格：3 列、9 个候选（3 行），高亮第一格。
fn matrix() -> Answer {
    let mut answer =
        Answer::composing("n", &["你", "呢", "尼", "泥", "拟", "逆", "妮", "倪", "腻"]);
    let frame = answer.frame.as_mut().expect("组句的一帧");
    frame.layout = LayoutMode::Horizontal;
    frame.columns = 3;
    frame.column_ems = vec![2.0; 3];
    frame.highlight = 0;
    answer
}

/// 点 `candidate4`：它在第二行第二列，报给 Server 的是视口里的下标 4，不是「第二个候选」。
#[test]
fn clicking_a_grid_cell_reports_its_viewport_index() {
    let Some(rig) = Rig::start(Setup::default()) else {
        return;
    };
    if !rig.frontend.painter() {
        skip("这台机器上没有界面字体，候选窗画不出来");
        return;
    }
    rig.server.allow(&["Candidate"]);
    rig.focus();
    rig.server.answer(matrix());
    let mark = rig.frontend.mark();
    rig.compositor.tap(KEY_N);
    let targets = rig.frontend.targets(mark);
    let (x0, y0) = targets.at("candidate0");
    let (x4, y4) = targets.at("candidate4");
    assert_ne!(y0, y4, "九宫里第 5 个在第二行：{targets:?}");
    assert_ne!(x0, x4, "九宫里第 5 个换了一列：{targets:?}");

    rig.compositor.enter(x4, y4);
    rig.compositor.click(BUTTON_LEFT);
    rig.server
        .received
        .wait(0, "点第 5 格", |message| kind(message) == "Candidate");
    let clicks = rig.server.pointer_events("Candidate");
    assert_eq!(clicks.len(), 1, "{clicks:?}");
    assert_eq!(clicks[0]["index"], 4);
}
