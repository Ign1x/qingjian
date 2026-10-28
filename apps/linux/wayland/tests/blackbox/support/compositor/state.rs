//! 假合成器的状态：客户端拿到的几个关键对象、发过几次 done、键位表，以及测试要它发的事件。

use std::fs::File;
use std::os::fd::AsFd;

use wayland_protocols_misc::zwp_input_method_v2::server::zwp_input_method_keyboard_grab_v2::ZwpInputMethodKeyboardGrabV2;
use wayland_protocols_misc::zwp_input_method_v2::server::zwp_input_method_v2::ZwpInputMethodV2;
use wayland_server::protocol::wl_keyboard::{KeyState, KeymapFormat};
use wayland_server::protocol::wl_pointer::WlPointer;
use wayland_server::protocol::wl_surface::WlSurface;

use super::{Request, Setup};
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
            dones: 0,
            pointer: None,
            serial: 0,
            time: 1000,
            modifiers: [0; 4],
            keymap,
            keymap_size,
        }
    }

    /// 输入框要输入法了：activate + done。客户端还没拿输入法对象时返回 `false`。
    pub fn activate(&mut self) -> bool {
        let Some(method) = &self.input_method else {
            return false;
        };
        method.activate();
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

    /// 这一批状态生效。应用 commit 了自己的 text-input 状态之后，合成器也转来一次。
    pub fn done(&mut self) -> bool {
        let Some(method) = &self.input_method else {
            return false;
        };
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

    /// 经 grab 把上一次的修饰键原样再发一次；没有 grab 是 `false`。
    pub fn resend_modifiers(&mut self) -> bool {
        let Some(grab) = &self.grab else {
            return false;
        };
        self.serial += 1;
        let [depressed, latched, locked, group] = self.modifiers;
        grab.modifiers(self.serial, depressed, latched, locked, group);
        true
    }

    /// 客户端接管了键盘：像 wlroots 那样马上发键位表、重复设置与修饰键。
    pub fn grabbed(&mut self, grab: ZwpInputMethodKeyboardGrabV2) {
        grab.keymap(KeymapFormat::XkbV1, self.keymap.as_fd(), self.keymap_size);
        grab.repeat_info(REPEAT_RATE, REPEAT_DELAY);
        self.grab = Some(grab);
        self.resend_modifiers();
    }
}
