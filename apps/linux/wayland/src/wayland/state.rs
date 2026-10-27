//! Wayland 一侧的全部状态：绑定的全局对象、输入法与键盘、候选窗，以及排给会话的事。各 Dispatch 落在这里。

use std::collections::VecDeque;
use std::time::Instant;

use qingjian_platform::protocol::Frame;
use wayland_client::QueueHandle;
use wayland_client::globals::GlobalList;
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::protocol::wl_shm::WlShm;
use wayland_protocols::wp::fractional_scale::v1::client::wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1;
use wayland_protocols::wp::viewporter::client::wp_viewporter::WpViewporter;
use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_keyboard_grab_v2::ZwpInputMethodKeyboardGrabV2;
use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_manager_v2::ZwpInputMethodManagerV2;
use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_v2::ZwpInputMethodV2;
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;
use wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1;

use super::keyboard::VirtualKeyboard;
use super::method::Method;
use super::popup::Popup;
use super::shm::Buffers;
use super::toplevel::Toplevels;
use super::xkb::Xkb;
use crate::error::Error;
use crate::session::Input;
use crate::view::Painter;

pub struct Wayland {
    pub qh: QueueHandle<Wayland>,

    pub shm: WlShm,

    pub input_method: ZwpInputMethodV2,

    /// 输入框那边的状态与焦点代次。
    pub method: Method,

    /// 活动窗口是哪个应用；合成器没有 foreign-toplevel 时一直不知道。
    pub toplevels: Toplevels,

    /// 键盘 grab；交还键盘时是 `None`。
    pub grab: Option<ZwpInputMethodKeyboardGrabV2>,

    /// grab 给的键位表；还没收到或编译不了是 `None`。
    pub xkb: Option<Xkb>,

    pub keyboard: VirtualKeyboard,

    pub popup: Popup,

    pub buffers: Buffers,

    /// 画候选窗的；字体库建不起来时是 `None`（只有行内拼音）。
    pub painter: Option<Painter>,

    /// 候选窗上的那一帧：缩放变了、主题改了要重画它。
    pub shown: Option<Frame>,

    /// 动画的下一帧什么时候画。
    pub animate_at: Option<Instant>,

    /// 下一次查配置与主题的时刻。
    pub reload_at: Instant,

    /// 排给会话的事，按到达顺序。
    pub inbox: VecDeque<Input>,

    /// 别的输入法占着这个 seat。
    pub unavailable: bool,
}

impl Wayland {
    /// 绑定要用的全局对象，建好输入法、虚拟键盘与候选窗。缺输入法或虚拟键盘的合成器（KWin、GNOME）直接报错。
    pub fn new(
        globals: &GlobalList,
        qh: &QueueHandle<Self>,
        painter: Option<Painter>,
    ) -> Result<Self, Error> {
        let compositor: WlCompositor = globals
            .bind(qh, 4..=6, ())
            .map_err(|_| Error::Missing("wl_compositor v4"))?;
        let shm: WlShm = globals
            .bind(qh, 1..=1, ())
            .map_err(|_| Error::Missing("wl_shm"))?;
        let seat: WlSeat = globals
            .bind(qh, 1..=1, ())
            .map_err(|_| Error::Missing("wl_seat"))?;
        let methods: ZwpInputMethodManagerV2 = globals
            .bind(qh, 1..=1, ())
            .map_err(|_| Error::Missing("zwp_input_method_manager_v2"))?;
        let keyboards: ZwpVirtualKeyboardManagerV1 = globals
            .bind(qh, 1..=1, ())
            .map_err(|_| Error::Missing("zwp_virtual_keyboard_manager_v1"))?;
        let viewporter: Option<WpViewporter> = globals.bind(qh, 1..=1, ()).ok();
        let fractional: Option<WpFractionalScaleManagerV1> = globals.bind(qh, 1..=1, ()).ok();
        if viewporter.is_none() || fractional.is_none() {
            tracing::info!("合成器没有分数缩放或 viewporter，候选窗按整数缩放画");
        }
        // 绑上之后合成器先把已有的窗口全报一遍，之后有变化再报
        let manager: Option<ZwlrForeignToplevelManagerV1> = globals.bind(qh, 1..=3, ()).ok();
        if manager.is_none() {
            tracing::info!(
                "合成器没有 zwlr_foreign_toplevel_manager_v1，认不出输入框在哪个应用里：所有应用共用一个会话，[apps] 按应用的设置不生效"
            );
        }
        let input_method = methods.get_input_method(&seat, qh, ());
        let keyboard = VirtualKeyboard::new(keyboards.create_virtual_keyboard(&seat, qh, ()));
        let popup = Popup::new(
            &compositor,
            &input_method,
            viewporter.as_ref(),
            fractional.as_ref(),
            qh,
        );
        Ok(Self {
            qh: qh.clone(),
            shm,
            input_method,
            method: Method::default(),
            toplevels: Toplevels::default(),
            grab: None,
            xkb: None,
            keyboard,
            popup,
            buffers: Buffers::default(),
            painter,
            shown: None,
            animate_at: None,
            reload_at: Instant::now(),
            inbox: VecDeque::new(),
            unavailable: false,
        })
    }
}
