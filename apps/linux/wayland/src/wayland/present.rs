//! 把帧画到候选窗上：渲染、与弹着的右键菜单合成、裁边补齐、贴图，以及动画、缩放与主题变化时的重画。

use std::time::{Duration, Instant};

use qingjian_platform::protocol::Frame;

use crate::view::{self, Canvas, blit};
use crate::wayland::state::Wayland;

/// 多久查一次配置、主题与桌面的外观偏好有没有变。
const RELOAD_INTERVAL: Duration = Duration::from_secs(1);

impl Wayland {
    /// 画出这一帧并贴上，返回画出来的译词义项。字体库没建起来时不画（只有行内拼音）。
    /// 换了一帧，弹着的右键菜单与排着的点击都对不上了（[`Self::drop_clicks`]），画没画成都作废。
    pub(super) fn show(&mut self, frame: &Frame) -> Vec<(usize, usize)> {
        self.menu = None;
        self.drop_clicks();
        self.shown = Some(frame.clone());
        self.hinted = None;
        let scale = self.popup.scale();
        let Some(rendered) = self
            .painter
            .as_mut()
            .and_then(|painter| painter.render(frame, scale as f32))
        else {
            // 保留待重画的新帧，但清掉旧画面与动画，避免后续 tick 恢复旧布局的点击区域。
            self.popup.hide();
            self.window = None;
            self.canvas = None;
            self.animate_at = None;
            self.repaint = false;
            if let Some(painter) = &mut self.painter {
                painter.forget();
            }
            return Vec::new();
        };
        let senses = view::senses(&rendered);
        self.window = Some(rendered);
        self.paint();
        senses
    }

    pub(super) fn hide(&mut self) {
        self.popup.hide();
        self.shown = None;
        self.hinted = None;
        self.window = None;
        self.menu = None;
        self.canvas = None;
        self.drop_clicks();
        self.animate_at = None;
        self.repaint = false;
        // 键盘画在候选窗那一个 popup 里，一起收起；侧边的粘性只在这一段输入里有效
        self.key_hint = None;
        self.keyboard = None;
        self.keyboard_side = None;
        if let Some(painter) = &mut self.painter {
            painter.forget();
        }
    }

    /// 到点的显示杂事：动画的下一帧；缩放变了、主题改了或桌面的外观偏好变了就重画当前帧。
    pub(super) fn tick(&mut self, now: Instant) {
        if self.animate_at.is_some_and(|at| at <= now) {
            self.animate_at = None;
            if let Some(rendered) = self.painter.as_mut().and_then(|painter| painter.tick()) {
                self.window = Some(rendered);
                self.paint();
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
        if redraw {
            self.redraw();
        }
        // 光标矩形跨过了 y = 0：合成器把 popup 翻到了另一边，键盘要换到另一侧。
        // 粘住的那一侧是按翻之前选的，先忘掉，按翻转后的顺序重选。
        if std::mem::take(&mut self.rekey) && self.keyboard.is_some() {
            self.keyboard_side = None;
            self.paint();
        }
        if self.repaint {
            self.paint();
        }
    }

    pub(super) fn deadline(&self) -> Option<Instant> {
        [self.animate_at, Some(self.reload_at)]
            .into_iter()
            .flatten()
            .min()
    }

    /// 按现在的缩放与外观重画候选或中 / 英提示；弹着的菜单、画着的键盘也重画，摆回原处。
    fn redraw(&mut self) {
        let scale = self.popup.scale() as f32;
        let hint = self.key_hint.clone();
        let Some(painter) = self.painter.as_mut() else {
            return;
        };
        let rendered = match (&self.shown, &self.hinted) {
            (Some(frame), _) => painter.render(frame, scale),
            (_, Some(mode)) => painter.render_status(mode, scale),
            _ => return,
        };
        let Some(rendered) = rendered else {
            return;
        };
        if let Some(menu) = &mut self.menu
            && let Some(again) = painter.render_menu(menu.items(), menu.hovered, scale)
        {
            menu.rendered = again;
        }
        // 缩放或主题变了，键盘按新的倍数、新的深浅重画
        if let Some(hint) = &hint
            && let Some(keyboard) = painter.render_key_hint(hint, scale)
        {
            self.keyboard = Some(keyboard);
        }
        self.window = Some(rendered);
        self.paint();
    }

    /// 把候选窗（连同右键菜单、键位提示）或中 / 英提示贴到 popup 上，鼠标只按贴上去的那一张认落点。
    pub(super) fn paint(&mut self) {
        let Some(window) = &self.window else {
            return;
        };
        let menu = self.menu.as_ref().map(|menu| (&menu.rendered, menu.anchor));
        let canvas = if self.hinted.is_some() {
            Canvas::hint(window, self.popup.scale())
        } else {
            let mut canvas = Canvas::new(window, menu, self.popup.scale());
            if let Some(keyboard) = &self.keyboard {
                let side =
                    canvas.place_key_hint(keyboard, self.popup.flipped(), self.keyboard_side);
                self.keyboard_side = Some(side);
            }
            canvas
        };
        let pixels = canvas.compose(
            window,
            menu.map(|(rendered, _)| rendered),
            self.keyboard.as_ref(),
        );
        let placement = canvas.placement;
        let Some((buffer_pixels, buffer)) =
            self.buffers
                .take(&self.shm, &self.qh, placement.width, placement.height)
        else {
            tracing::debug!("缓冲都还在合成器手里，这一帧跳过");
            // 同上：贴着的不是这一帧，先不收点击
            self.canvas = None;
            self.repaint = true;
            return;
        };
        blit(&pixels, canvas.width(), &placement, buffer_pixels);
        self.popup
            .attach(&buffer, &placement, &canvas.input_region(), &self.qh);
        self.animate_at = window.next_frame.map(|delay| Instant::now() + delay);
        if tracing::enabled!(tracing::Level::DEBUG) {
            let targets = canvas.describe();
            if self
                .canvas
                .as_ref()
                .is_none_or(|old| old.describe() != targets)
            {
                tracing::debug!(%targets, "候选窗上能点的地方（surface 坐标）");
            }
        }
        self.canvas = Some(canvas);
        self.repaint = false;
    }
}
