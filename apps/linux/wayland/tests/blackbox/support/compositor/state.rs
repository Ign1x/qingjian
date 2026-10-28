//! 假合成器的状态：客户端拿到的几个关键对象、发过几次 done、键位表，以及测试要它发的事件。

use std::fs::File;
use std::os::fd::AsFd;

use wayland_protocols_misc::zwp_input_method_v2::server::zwp_input_method_keyboard_grab_v2::ZwpInputMethodKeyboardGrabV2;
use wayland_protocols_misc::zwp_input_method_v2::server::zwp_input_method_v2::ZwpInputMethodV2;
use wayland_protocols_misc::zwp_input_method_v2::server::zwp_input_popup_surface_v2::ZwpInputPopupSurfaceV2;
use wayland_server::protocol::wl_keyboard::{KeyState, KeymapFormat};
use wayland_server::protocol::wl_pointer::WlPointer;
use wayland_server::protocol::wl_surface::WlSurface;

use super::{Field, Request, Setup};
use crate::support::Log;

/// 按住多久开始重复（毫秒）、每秒几次：grab 建好时与真合成器一样发一次。
const REPEAT_DELAY: i32 = 600;
const REPEAT_RATE: i32 = 25;

pub struct State {
    pub requests: Log<Request>,

    pub setup: Setup,

    /// 客户端拿的输入法对象。
    pub input_method: Option<ZwpInputMethodV2>,

    /// 键盘 grab；交还了是 `None`。
    pub grab: Option<ZwpInputMethodKeyboardGrabV2>,

    /// 当了候选窗的那个 surface。
    pub popup: Option<WlSurface>,

    /// 候选窗的 input-popup-surface 角色：合成器往客户端发光标位置（`text_input_rectangle`）用它。
    pub input_popup: Option<ZwpInputPopupSurfaceV2>,

    /// 应用那边输入框现在的状态，每次 done 之前转给输入法。
    pub field: Field,

    /// 发过几次 done：输入法 commit 的 serial 该等于它。
    pub dones: u32,

    /// 客户端从 seat 拿的指针；还没拿或放掉了是 `None`。
    pub pointer: Option<WlPointer>,

    /// 按键、修饰键与指针事件的 serial。
    pub(super) serial: u32,

    /// 合成器的时间轴（毫秒），按键与指针事件的时间戳从这里取。
    pub(super) time: u32,

    /// 最近一次发的修饰键：depressed、latched、locked、group。
    modifiers: [u32; 4],

    /// 这次 grab 上发过几次修饰键：被测进程拿着键位表时一次不落地转给虚拟键盘，[`super::Compositor::sync`] 按它数。
    pub sent: usize,

    /// grab 发的键位表：测试夹具连同结尾的 0。
    keymap: File,

    keymap_size: u32,
}

impl State {
    pub fn new(requests: Log<Request>, setup: Setup, keymap: File, keymap_size: u32) -> Self {
        Self {
            requests,
            setup,
            input_method: None,
            grab: None,
            popup: None,
            input_popup: None,
            field: Field::default(),
            dones: 0,
            pointer: None,
            serial: 0,
            time: 1000,
            modifiers: [0; 4],
            sent: 0,
            keymap,
            keymap_size,
        }
    }

    /// 输入框要输入法了：activate，连同这个输入框的状态 + done。客户端还没拿输入法对象时返回 `false`。
    pub fn activate(&mut self, field: Field) -> bool {
        let Some(method) = &self.input_method else {
            return false;
        };
        method.activate();
        self.field = field;
        self.done()
    }

    /// 激活着的输入框报了新状态（应用 commit 了自己的 text-input 状态）：转给输入法 + done。
    pub fn update(&mut self, field: Field) -> bool {
        self.field = field;
        self.done()
    }

    /// 输入框不要输入法了：deactivate + done。
    pub fn deactivate(&mut self) -> bool {
        let Some(method) = &self.input_method else {
            return false;
        };
        method.deactivate();
        self.done()
    }

    /// 这一批状态生效：先转一遍输入框的状态，再 done。应用 commit 了自己的 text-input 状态之后，合成器也转来一次。
    pub fn done(&mut self) -> bool {
        let Some(method) = &self.input_method else {
            return false;
        };
        self.field.send(method);
        // 变化原因只管它所在的那一次 commit，之后回到 input_method（text-input-v3 的约定）
        self.field.changed_by_app = false;
        method.done();
        self.dones += 1;
        true
    }

    /// 经 grab 发一次按下或抬起，返回它的时间戳；没有 grab 是 `None`。
    pub fn key(&mut self, code: u32, pressed: bool) -> Option<u32> {
        let grab = self.grab.as_ref()?;
        self.time += 10;
        self.serial += 1;
        let state = if pressed {
            KeyState::Pressed
        } else {
            KeyState::Released
        };
        grab.key(self.serial, self.time, code, state);
        Some(self.time)
    }

    /// 经 grab 按下或抬起一个修饰键：先发键，再发按着的修饰键（`mask` 是它在 xkb 里的修饰键位，与 wlroots 的先后一样）。
    /// 没有 grab 是 `None`。
    pub fn modifier(&mut self, code: u32, mask: u32, pressed: bool) -> Option<u32> {
        let time = self.key(code, pressed)?;
        if pressed {
            self.modifiers[0] |= mask;
        } else {
            self.modifiers[0] &= !mask;
        }
        self.resend_modifiers();
        Some(time)
    }

    /// 经 grab 把上一次的修饰键原样再发一次；没有 grab 是 `false`。
    pub fn resend_modifiers(&mut self) -> bool {
        let Some(grab) = &self.grab else {
            return false;
        };
        self.serial += 1;
        let [depressed, latched, locked, group] = self.modifiers;
        grab.modifiers(self.serial, depressed, latched, locked, group);
        self.sent += 1;
        true
    }

    /// 客户端接管了键盘：像 wlroots 那样马上发键位表、重复设置与修饰键。
    pub fn grabbed(&mut self, grab: ZwpInputMethodKeyboardGrabV2) {
        grab.keymap(KeymapFormat::XkbV1, self.keymap.as_fd(), self.keymap_size);
        grab.repeat_info(REPEAT_RATE, REPEAT_DELAY);
        self.grab = Some(grab);
        self.sent = 0;
        self.resend_modifiers();
    }
}
