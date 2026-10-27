//! 协议帧 → 渲染器的帧：自绘候选窗的壳共用的那一步（`render` 特性）。
//!
//! Server 发的 [`crate::protocol::Frame`] 是「现在该显示什么」，[`qingjian_render::Frame`] 是「这一窗怎么画」：
//! 候选折成带序号与注记片段的行、拼音行按 `[general] preedit` 决定画不画、页码写成 `1/6`。
//! 这一步各个自绘的壳本来各写一份（见 `docs/design/core-internals.md`「还是各写一份的地方」），
//! 这里是 Linux Wayland 前端用的那份，照 Windows Server 的 `ui/candidates/{row,render_data}.rs` 写；Windows 与 macOS 以后改用它。

mod preedit;
mod row;

#[cfg(test)]
mod tests;

pub use preedit::window_preedit;
pub use row::from_candidate;

use qingjian_render::{Layout, Mode};

use crate::LayoutMode;
use crate::protocol::Frame;

/// 一帧要画的全部内容。`mode` 是输入状态（中 / 英、简 / 繁、方案），主题拿它显示状态；协议帧里没有，由壳给。
pub fn render_frame(frame: &Frame, mode: Mode) -> qingjian_render::Frame {
    let grid = frame.columns > 0;
    // 矩阵里序号只标在高亮所在那一行（数字键选的就是它），空位什么都不画；单行照旧全标
    let highlight_row = grid.then(|| frame.highlight / frame.columns);
    let rows = frame
        .candidates
        .items
        .iter()
        .enumerate()
        .map(|(i, candidate)| {
            let position = if grid { i % frame.columns } else { i };
            let mut row = from_candidate(
                position,
                candidate,
                frame.aux_code_show,
                frame.translation_pos,
            );
            if grid && (Some(i / frame.columns) != highlight_row || candidate.text.is_empty()) {
                row.index = String::new();
            }
            row
        })
        .collect();
    qingjian_render::Frame {
        preedit: window_preedit(frame),
        rows,
        // 协议里 usize::MAX 表示不高亮（拼音解析不出来的原样输入）
        highlighted: (frame.highlight != usize::MAX).then_some(frame.highlight),
        columns: frame.columns,
        column_ems: frame.column_ems.clone(),
        footer: (frame.page_count > 1).then(|| format!("{}/{}", frame.page + 1, frame.page_count)),
        sentence: frame.sentence.clone(),
        status: frame.notice.clone(),
        mode,
    }
}

/// 配置里的候选排布 → 渲染器的排布。
pub fn layout(layout: LayoutMode) -> Layout {
    match layout {
        LayoutMode::Vertical => Layout::Vertical,
        LayoutMode::Horizontal => Layout::Horizontal,
    }
}
