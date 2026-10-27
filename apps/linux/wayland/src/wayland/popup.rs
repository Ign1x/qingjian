//! 候选窗：input-method-v2 的 popup surface。位置由合成器定（贴在光标旁），这里只管贴位图与缩放。

use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_surface::{self, WlSurface};
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::wp::fractional_scale::v1::client::wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1;
use wayland_protocols::wp::fractional_scale::v1::client::wp_fractional_scale_v1::{
    self, WpFractionalScaleV1,
};
use wayland_protocols::wp::viewporter::client::wp_viewport::WpViewport;
use wayland_protocols::wp::viewporter::client::wp_viewporter::WpViewporter;
use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_v2::ZwpInputMethodV2;
use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_popup_surface_v2::{
    self, ZwpInputPopupSurfaceV2,
};

use super::Wayland;
use crate::view::Placement;

pub struct Popup {
    surface: WlSurface,

    _role: ZwpInputPopupSurfaceV2,

    /// 有分数缩放时用它定逻辑尺寸；没有时退回整数的 buffer scale。
    viewport: Option<WpViewport>,

    _fractional: Option<WpFractionalScaleV1>,

    /// 合成器希望的缩放；第一次贴图之前不知道，按上一次的（起初是 1）。
    scale: f64,

    /// 缩放变了，当前这一帧要按新缩放重画。
    pub rescaled: bool,

    /// 挂着缓冲（显示着）。
    mapped: bool,
}

impl Popup {
    pub fn new(
        compositor: &WlCompositor,
        input_method: &ZwpInputMethodV2,
        viewporter: Option<&WpViewporter>,
        fractional: Option<&WpFractionalScaleManagerV1>,
        qh: &QueueHandle<Wayland>,
    ) -> Self {
        let surface = compositor.create_surface(qh, ());
        // 输入区域设成空，鼠标穿过候选窗：第一轮还不接鼠标点选
        let region = compositor.create_region(qh, ());
        surface.set_input_region(Some(&region));
        region.destroy();
        let role = input_method.get_input_popup_surface(&surface, qh, ());
        let (viewport, fractional) = match (viewporter, fractional) {
            (Some(viewporter), Some(fractional)) => (
                Some(viewporter.get_viewport(&surface, qh, ())),
                Some(fractional.get_fractional_scale(&surface, qh, ())),
            ),
            _ => (None, None),
        };
        Self {
            surface,
            _role: role,
            viewport,
            _fractional: fractional,
            scale: 1.0,
            rescaled: false,
            mapped: false,
        }
    }

    /// 画位图用的缩放。没有分数缩放时是整数，[`Placement`] 据此把缓冲补齐到整倍数。
    pub fn scale(&self) -> f64 {
        self.scale
    }

    /// 贴上一块画好的缓冲。有 viewport 时 buffer scale 保持 1、按逻辑尺寸设目标大小
    /// （Hyprland 拿「viewport 尺寸 ÷ buffer scale」当 popup 尺寸摆位置，两样一起设会算错）。
    pub fn attach(&mut self, buffer: &WlBuffer, placement: &Placement) {
        self.surface.attach(Some(buffer), 0, 0);
        self.surface
            .damage_buffer(0, 0, placement.width as i32, placement.height as i32);
        match &self.viewport {
            Some(viewport) => {
                viewport.set_destination(placement.logical_width, placement.logical_height);
            }
            None => self.surface.set_buffer_scale(self.scale as i32),
        }
        self.surface.commit();
        self.mapped = true;
    }

    /// 收起：挂空缓冲。失活时也要收，不然下次激活时合成器会把旧候选先亮一下。
    pub fn hide(&mut self) {
        if self.mapped {
            self.surface.attach(None, 0, 0);
            self.surface.commit();
            self.mapped = false;
        }
    }

    fn set_scale(&mut self, scale: f64) {
        if scale > 0.0 && (scale - self.scale).abs() > f64::EPSILON {
            tracing::debug!(scale, "候选窗缩放变了");
            self.scale = scale;
            self.rescaled = self.mapped;
        }
    }
}

impl Dispatch<WpFractionalScaleV1, ()> for Wayland {
    fn event(
        state: &mut Self,
        _: &WpFractionalScaleV1,
        event: wp_fractional_scale_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event {
            // 分数缩放按 120 分之几给
            state.popup.set_scale(f64::from(scale) / 120.0);
        }
    }
}

impl Dispatch<WlSurface, ()> for Wayland {
    fn event(
        state: &mut Self,
        _: &WlSurface,
        event: wl_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_surface::Event::PreferredBufferScale { factor } = event
            && state.popup.viewport.is_none()
        {
            state.popup.set_scale(f64::from(factor));
        }
    }
}

impl Dispatch<ZwpInputPopupSurfaceV2, ()> for Wayland {
    fn event(
        _: &mut Self,
        _: &ZwpInputPopupSurfaceV2,
        event: zwp_input_popup_surface_v2::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // 光标相对候选窗的位置：从它能看出合成器有没有把候选窗翻到光标上方（以后按它调排布）
        if let zwp_input_popup_surface_v2::Event::TextInputRectangle {
            x,
            y,
            width,
            height,
        } = event
        {
            tracing::trace!(x, y, width, height, "光标相对候选窗的位置");
        }
    }
}
