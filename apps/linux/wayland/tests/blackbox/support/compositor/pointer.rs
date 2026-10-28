//! 指针：seat 上的 wl_pointer 与 cursor-shape。测试经 [`Compositor`] 往候选窗发 enter / motion / button / axis，
//! 每一批后面跟一个 frame（第 5 版起的规矩）；客户端设的光标形状记下来。

use wayland_protocols::wp::cursor_shape::v1::server::wp_cursor_shape_device_v1::{
    self, WpCursorShapeDeviceV1,
};
use wayland_protocols::wp::cursor_shape::v1::server::wp_cursor_shape_manager_v1::{
    self, WpCursorShapeManagerV1,
};
use wayland_server::protocol::wl_pointer::{self, Axis, AxisSource, ButtonState, WlPointer};
use wayland_server::{Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, WEnum};

use super::Compositor;
use crate::support::compositor::request::Request;
use crate::support::compositor::state::State;

/// evdev 的左键与右键（`linux/input-event-codes.h`）。
pub const BUTTON_LEFT: u32 = 0x110;
pub const BUTTON_RIGHT: u32 = 0x111;

impl State {
    /// 指针进了候选窗，停在 surface 上的 `(x, y)`（逻辑像素）。客户端还没拿指针、候选窗还没建是 `false`。
    fn enter(&mut self, x: f64, y: f64) -> bool {
        let (Some(pointer), Some(popup)) = (&self.pointer, &self.popup) else {
            return false;
        };
        self.serial += 1;
        pointer.enter(self.serial, popup, x, y);
        pointer.frame();
        true
    }

    /// 客户端拿的指针，与下一个事件的 serial 和时间戳。
    fn pointer(&mut self) -> (&WlPointer, u32, u32) {
        self.time += 10;
        self.serial += 1;
        let pointer = self.pointer.as_ref().expect("被测进程没拿指针");
        (pointer, self.serial, self.time)
    }

    /// 在指针停着的地方按下再抬起一个键，各跟一个 frame。
    fn click(&mut self, button: u32) {
        for pressed in [ButtonState::Pressed, ButtonState::Released] {
            let (pointer, serial, time) = self.pointer();
            pointer.button(serial, time, button, pressed);
            pointer.frame();
        }
    }
}

impl Compositor {
    /// 指针进候选窗、停在 `(x, y)`（surface 逻辑像素）。
    pub fn enter(&self, x: f64, y: f64) {
        assert!(
            self.call(move |state| state.enter(x, y)),
            "被测进程没拿指针，或候选窗还没建"
        );
    }

    /// 指针在候选窗上移到 `(x, y)`。
    pub fn motion(&self, x: f64, y: f64) {
        self.call(move |state| {
            let (pointer, _, time) = state.pointer();
            pointer.motion(time, x, y);
            pointer.frame();
        });
    }

    /// 在指针停着的地方按下再抬起一个键。
    pub fn click(&self, button: u32) {
        self.call(move |state| state.click(button));
    }

    /// 多次移动与点击一批送出，用来复现事件排队期间菜单变化。
    pub fn clicks(&self, clicks: &[(f64, f64, u32)]) {
        let clicks = clicks.to_vec();
        self.call(move |state| {
            for (x, y, button) in clicks {
                let (pointer, _, time) = state.pointer();
                pointer.motion(time, x, y);
                pointer.frame();
                state.click(button);
            }
        });
    }

    /// 按下键盘上的 `code`、紧跟着在指针停着的地方点一下 `button`、再抬起 `code`，一起送出去：
    /// 被测进程处理到这一下点击时，前面那个键已经换上了新的一帧。
    pub fn type_and_click(&self, code: u32, button: u32) {
        let sent = self.call(move |state| {
            let pressed = state.key(code, true);
            state.click(button);
            pressed.and(state.key(code, false))
        });
        sent.expect("被测进程没有接管键盘");
    }

    /// 滚轮滚 `value120` 个 120 分之一格（正的往下）：与真合成器一样，同一下的格数与距离放在一帧里。
    pub fn wheel(&self, value120: i32) {
        self.call(move |state| {
            let (pointer, _, time) = state.pointer();
            pointer.axis_source(AxisSource::Wheel);
            pointer.axis_value120(Axis::VerticalScroll, value120);
            pointer.axis(time, Axis::VerticalScroll, f64::from(value120) / 8.0);
            pointer.frame();
        });
    }

    /// 触摸板上两指滑了 `distance`（逻辑像素，正的往下），一帧。
    pub fn swipe(&self, distance: f64) {
        self.call(move |state| {
            let (pointer, _, time) = state.pointer();
            pointer.axis_source(AxisSource::Finger);
            pointer.axis(time, Axis::VerticalScroll, distance);
            pointer.frame();
        });
    }
}

impl Dispatch<WlPointer, ()> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &WlPointer,
        request: wl_pointer::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        if let wl_pointer::Request::Release = request {
            state.pointer = None;
        }
    }
}

impl GlobalDispatch<WpCursorShapeManagerV1, ()> for State {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        resource: New<WpCursorShapeManagerV1>,
        _: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<WpCursorShapeManagerV1, ()> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &WpCursorShapeManagerV1,
        request: wp_cursor_shape_manager_v1::Request,
        _: &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let wp_cursor_shape_manager_v1::Request::GetPointer {
            cursor_shape_device,
            ..
        } = request
        {
            data_init.init(cursor_shape_device, ());
        }
    }
}

impl Dispatch<WpCursorShapeDeviceV1, ()> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &WpCursorShapeDeviceV1,
        request: wp_cursor_shape_device_v1::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        if let wp_cursor_shape_device_v1::Request::SetShape { serial, shape } = request {
            let shape = match shape {
                WEnum::Value(shape) => u32::from(shape),
                WEnum::Unknown(value) => value,
            };
            state.requests.push(Request::CursorShape { serial, shape });
        }
    }
}
