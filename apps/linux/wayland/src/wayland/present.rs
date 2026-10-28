//! 把帧画到候选窗上：渲染、裁边补齐、贴图，以及动画、缩放与主题变化时的重画。

use std::time::{Duration, Instant};

use qingjian_platform::protocol::Frame;
use qingjian_render::Rendered;

use super::Wayland;
use crate::view::{self, Placement, blit};

/// 多久查一次配置、主题与桌面的外观偏好有没有变。
const RELOAD_INTERVAL: Duration = Duration::from_secs(1);

impl Wayland {
    /// 画出这一帧并贴上，返回画出来的译词义项。字体库没建起来时不画（只有行内拼音）。
    pub(super) fn show(&mut self, frame: &Frame) -> Vec<(usize, usize)> {
        let scale = self.popup.scale();
        let Some(rendered) = self
            .painter
            .as_mut()
            .and_then(|painter| painter.render(frame, scale as f32))
        else {
            return Vec::new();
        };
        self.shown = Some(frame.clone());
        self.paint(&rendered);
        view::senses(&rendered)
    }

    pub(super) fn hide(&mut self) {
        self.popup.hide();
        self.shown = None;
        self.animate_at = None;
        if let Some(painter) = &mut self.painter {
            painter.forget();
        }
    }

    /// 到点的显示杂事：动画的下一帧；缩放变了、主题改了或桌面的外观偏好变了就重画当前帧。
    pub(super) fn tick(&mut self, now: Instant) {
        if self.animate_at.is_some_and(|at| at <= now) {
            self.animate_at = None;
            if let Some(rendered) = self.painter.as_mut().and_then(|painter| painter.tick()) {
                self.paint(&rendered);
            }
        }
        let mut redraw = std::mem::take(&mut self.popup.rescaled);
        if self.reload_at <= now {
            self.reload_at = now + RELOAD_INTERVAL;
            redraw |= self
                .painter
                .as_mut()
                .is_some_and(|painter| painter.reload());
        }
        if redraw && let Some(frame) = self.shown.clone() {
            self.show(&frame);
        }
    }

    pub(super) fn deadline(&self) -> Option<Instant> {
        [self.animate_at, Some(self.reload_at)]
            .into_iter()
            .flatten()
            .min()
    }

    pub(super) fn paint(&mut self, rendered: &Rendered) {
        let pixmap = &rendered.pixmap;
        let placement = Placement::new(
            pixmap.width(),
            pixmap.height(),
            rendered.content_x,
            rendered.content_y,
            self.popup.scale(),
        );
        let Some((pixels, buffer)) =
            self.buffers
                .take(&self.shm, &self.qh, placement.width, placement.height)
        else {
            tracing::debug!("缓冲都还在合成器手里，这一帧跳过");
            return;
        };
        blit(pixmap.data(), pixmap.width(), &placement, pixels);
        self.popup.attach(&buffer, &placement);
        self.animate_at = rendered.next_frame.map(|delay| Instant::now() + delay);
    }
}
