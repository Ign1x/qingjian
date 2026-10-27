//! 缩放：viewporter 记下候选窗的目标尺寸，分数缩放按 [`super::Setup::scale`] 告诉客户端合成器想要的倍数。

use std::sync::{Mutex, PoisonError};

use wayland_protocols::wp::fractional_scale::v1::server::wp_fractional_scale_manager_v1::{
    self, WpFractionalScaleManagerV1,
};
use wayland_protocols::wp::fractional_scale::v1::server::wp_fractional_scale_v1::{
    self, WpFractionalScaleV1,
};
use wayland_protocols::wp::viewporter::server::wp_viewport::{self, WpViewport};
use wayland_protocols::wp::viewporter::server::wp_viewporter::{self, WpViewporter};
use wayland_server::protocol::wl_surface::WlSurface;
use wayland_server::{Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource};

use super::{State, Surface};

impl GlobalDispatch<WpViewporter, ()> for State {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        resource: New<WpViewporter>,
        _: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<WpViewporter, ()> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &WpViewporter,
        request: wp_viewporter::Request,
        _: &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let wp_viewporter::Request::GetViewport { id, surface } = request {
            data_init.init(id, surface);
        }
    }
}

/// 用户数据是它管的那个 surface。
impl Dispatch<WpViewport, WlSurface> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &WpViewport,
        request: wp_viewport::Request,
        surface: &WlSurface,
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        if let wp_viewport::Request::SetDestination { width, height } = request
            && let Some(surface) = surface.data::<Mutex<Surface>>()
        {
            // -1 × -1 是取消目标尺寸
            surface
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .destination = (width > 0 && height > 0).then_some((width, height));
        }
    }
}

impl GlobalDispatch<WpFractionalScaleManagerV1, ()> for State {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        resource: New<WpFractionalScaleManagerV1>,
        _: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<WpFractionalScaleManagerV1, ()> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &WpFractionalScaleManagerV1,
        request: wp_fractional_scale_manager_v1::Request,
        _: &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let wp_fractional_scale_manager_v1::Request::GetFractionalScale { id, .. } = request {
            let scale = data_init.init(id, ());
            if let Some(preferred) = state.setup.scale {
                scale.preferred_scale(preferred);
            }
        }
    }
}

impl Dispatch<WpFractionalScaleV1, ()> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &WpFractionalScaleV1,
        _: wp_fractional_scale_v1::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
    }
}
