//! 热加载后同步分页与矩阵视口，撤销旧面板事件的展示身份。

use qingjian_core::CandidateLayout;

use crate::dispatch::Router;
use crate::dispatch::composed::Composed;

impl Router {
    pub(super) fn reload_display(&mut self) {
        if repage(
            &mut self.composed,
            &mut self.highlight,
            self.config.page_size,
            self.config.cloud_slots,
        ) {
            self.grid = None;
        }
        for info in self.sessions.values_mut() {
            if repage(
                &mut info.composed,
                &mut info.highlight,
                self.config.page_size,
                self.config.cloud_slots,
            ) {
                info.grid = None;
            }
            // 配置可改变帧中格号的含义；下一帧尚未送出时也必须拒绝旧鼠标事件。
            if let Some(identity) = &mut info.display_identity {
                self.display_revision += 1;
                identity.revision = self.display_revision;
            }
            info.display_frame = None;
            info.last_frame = None;
        }
        self.engine.note_displayed(std::iter::empty());
        self.collapse_disabled_grids();
    }
}

fn repage(
    composed: &mut Option<Composed>,
    highlight: &mut usize,
    page_size: usize,
    slots: usize,
) -> bool {
    let Some(Composed::Candidates { layout, .. }) = composed else {
        return false;
    };
    if layout.page_size() == page_size {
        return false;
    }
    let selected = layout.candidate(*highlight).cloned();
    let mut next = CandidateLayout::new(layout.local().to_vec(), page_size, slots);
    if layout.cloud_first() {
        next.set_cloud_first(layout.cloud().to_vec());
    } else {
        next.set_cloud(layout.cloud().to_vec());
    }
    *highlight = (0..next.len())
        .find(|&index| {
            next.candidate(index).is_some() && next.candidate(index) == selected.as_ref()
        })
        .unwrap_or_else(|| {
            (0..next.len())
                .find(|&index| next.candidate(index).is_some())
                .unwrap_or(0)
        });
    *layout = next;
    true
}
