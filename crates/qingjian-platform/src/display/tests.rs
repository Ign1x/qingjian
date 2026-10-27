//! 整帧换算：高亮、页码、提示与矩阵序号。

use qingjian_core::{Candidate, CandidateKind, CandidateList};
use qingjian_render::Mode;

use super::render_frame;
use crate::protocol::Frame;

fn candidate(text: &str) -> Candidate {
    Candidate {
        text: text.to_owned(),
        kind: CandidateKind::Chinese,
        syllables: Vec::new(),
        reading: None,
        translation: None,
        aux_code: None,
    }
}

fn frame(texts: &[&str]) -> Frame {
    Frame {
        candidates: CandidateList {
            items: texts.iter().map(|text| candidate(text)).collect(),
        },
        ..Frame::default()
    }
}

#[test]
fn page_footer_highlight_and_notice() {
    let mut single = frame(&["你好", "拟好"]);
    single.page_count = 1;
    single.highlight = 1;
    single.notice = Some("已删除 拟好".to_owned());
    let rendered = render_frame(&single, Mode::default());
    assert_eq!(rendered.highlighted, Some(1));
    assert_eq!(rendered.footer, None, "只有一页不画页码");
    assert_eq!(rendered.status.as_deref(), Some("已删除 拟好"));
    assert_eq!(
        rendered
            .rows
            .iter()
            .map(|row| row.index.as_str())
            .collect::<Vec<_>>(),
        ["1", "2"]
    );

    let mut paged = frame(&["你好"]);
    paged.page = 1;
    paged.page_count = 6;
    paged.highlight = usize::MAX;
    let rendered = render_frame(&paged, Mode::default());
    assert_eq!(rendered.footer.as_deref(), Some("2/6"));
    assert_eq!(rendered.highlighted, None, "usize::MAX 是不高亮");
}

/// 矩阵里序号只标高亮那一行，空位不标。
#[test]
fn grid_labels_only_the_highlighted_row() {
    let mut grid = frame(&["一", "二", "三", "四", "五", ""]);
    grid.columns = 3;
    grid.highlight = 4;
    let rendered = render_frame(&grid, Mode::default());
    let labels: Vec<_> = rendered.rows.iter().map(|row| row.index.as_str()).collect();
    assert_eq!(labels, ["", "", "", "1", "2", ""]);
}
