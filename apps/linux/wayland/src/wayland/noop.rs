//! 不用处理事件的那些对象。

use wayland_client::globals::GlobalListContents;
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_region::WlRegion;
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::protocol::wl_shm::WlShm;
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::{Connection, Dispatch, QueueHandle, delegate_noop};
use wayland_protocols::wp::fractional_scale::v1::client::wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1;
use wayland_protocols::wp::viewporter::client::wp_viewport::WpViewport;
use wayland_protocols::wp::viewporter::client::wp_viewporter::WpViewporter;
use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_manager_v2::ZwpInputMethodManagerV2;
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1;

use super::Wayland;

delegate_noop!(Wayland: ignore WlCompositor);
delegate_noop!(Wayland: ignore WlRegion);
delegate_noop!(Wayland: ignore WlSeat);
delegate_noop!(Wayland: ignore WlShm);
delegate_noop!(Wayland: ignore WlShmPool);
delegate_noop!(Wayland: ignore WpViewporter);
delegate_noop!(Wayland: ignore WpViewport);
delegate_noop!(Wayland: ignore WpFractionalScaleManagerV1);
delegate_noop!(Wayland: ignore ZwpInputMethodManagerV2);
delegate_noop!(Wayland: ignore ZwpVirtualKeyboardManagerV1);
delegate_noop!(Wayland: ignore ZwpVirtualKeyboardV1);

/// 启动之后新增、移除的全局对象不理：要用的在启动时都绑好了。
impl Dispatch<WlRegistry, GlobalListContents> for Wayland {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
