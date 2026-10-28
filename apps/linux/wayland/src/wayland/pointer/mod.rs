//! 鼠标：从 wl_seat 拿 wl_pointer（seat 的能力变了跟着拿或放），只管落在候选窗上的事件。
//! 左键、右键的按下按贴上去的那一张（[`Canvas`](crate::view::Canvas)）认出按在什么上，排成 [`Input::Click`]；
//! 滚轮按 [`Wheel`] 折成一页一页的 [`Input::Scroll`]；右键菜单开着时鼠标移动换悬停项。
//! 进候选窗时合成器有 cursor-shape 就把光标设成默认的箭头，没有就不设（光标随合成器）。

mod wheel;

use wayland_client::protocol::wl_pointer::{self, Axis, ButtonState, WlPointer};
use wayland_client::protocol::wl_seat::{self, Capability, WlSeat};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, WEnum};
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::{
    Shape, WpCursorShapeDeviceV1,
};
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_manager_v1::WpCursorShapeManagerV1;

use wheel::Wheel;

use super::Wayland;
use crate::session::{Click, Input};

/// evdev 的左键与右键（`linux/input-event-codes.h` 的 `BTN_LEFT` / `BTN_RIGHT`）。
const BUTTON_LEFT: u32 = 0x110;
const BUTTON_RIGHT: u32 = 0x111;

/// seat 上的指针设备。
pub struct Pointer {
    proxy: WlPointer,

    /// 设光标形状用；合成器没有 cursor-shape 时是 `None`。
    shape: Option<WpCursorShapeDeviceV1>,

    /// 指针在候选窗上的位置（surface 逻辑像素）；不在候选窗上是 `None`。
    pub position: Option<(f64, f64)>,

    /// 最近一次在候选窗上按下的位置：右键菜单摆在这里。
    pub pressed_at: Option<(f64, f64)>,

    wheel: Wheel,
}

impl Pointer {
    fn new(
        proxy: WlPointer,
        shapes: Option<&WpCursorShapeManagerV1>,
        qh: &QueueHandle<Wayland>,
    ) -> Self {
        let shape = shapes.map(|shapes| shapes.get_pointer(&proxy, qh, ()));
        Self {
            proxy,
            shape,
            position: None,
            pressed_at: None,
            wheel: Wheel::default(),
        }
    }

    /// seat 上没有指针设备了：放掉。
    fn release(self) {
        if let Some(shape) = self.shape {
            shape.destroy();
        }
        // release 是第 3 版才有的；更老的只能留着，合成器那边它已经没用了
        if self.proxy.version() >= 3 {
            self.proxy.release();
        }
    }
}

impl Wayland {
    /// 按下了左键或右键：按在候选窗上就认出按在什么上，排给会话。
    fn press(&mut self, button: u32) {
        let right = match button {
            BUTTON_LEFT => false,
            BUTTON_RIGHT => true,
            _ => return,
        };
        let Some(pointer) = &mut self.pointer else {
            return;
        };
        let Some((x, y)) = pointer.position else {
            return;
        };
        pointer.pressed_at = Some((x, y));
        let spot = self.canvas.as_ref().and_then(|canvas| canvas.spot(x, y));
        tracing::debug!(right, ?spot, "鼠标按在候选窗上");
        self.inbox.push_back(Input::Click(Click {
            right,
            spot,
            epoch: self.method.epoch,
        }));
    }

    /// 一帧滚动结束：够一页就排给会话。
    fn scrolled(&mut self) {
        let Some(pointer) = &mut self.pointer else {
            return;
        };
        if let Some(next) = pointer.wheel.frame()
            && pointer.position.is_some()
        {
            self.inbox.push_back(Input::Scroll {
                next,
                epoch: self.method.epoch,
            });
        }
    }

    /// 对着滚轮的累计做一件事；第 5 版之前没有 `frame` 事件，每个事件自成一帧。
    fn wheel(&mut self, pointer: &WlPointer, event: impl FnOnce(&mut Wheel)) {
        if let Some(state) = &mut self.pointer {
            event(&mut state.wheel);
        }
        if pointer.version() < 5 {
            self.scrolled();
        }
    }
}

impl Dispatch<WlSeat, ()> for Wayland {
    fn event(
        state: &mut Self,
        seat: &WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let wl_seat::Event::Capabilities { capabilities } = event else {
            return;
        };
        let capabilities = match capabilities {
            WEnum::Value(capabilities) => capabilities,
            WEnum::Unknown(bits) => Capability::from_bits_truncate(bits),
        };
        if capabilities.contains(Capability::Pointer) {
            if state.pointer.is_none() {
                let proxy = seat.get_pointer(qh, ());
                state.pointer = Some(Pointer::new(proxy, state.cursor_shapes.as_ref(), qh));
            }
        } else if let Some(pointer) = state.pointer.take() {
            pointer.release();
            state.hover();
        }
    }
}

impl Dispatch<WlPointer, ()> for Wayland {
    fn event(
        state: &mut Self,
        proxy: &WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter {
                serial,
                surface,
                surface_x,
                surface_y,
            } => {
                if !state.popup.is(&surface) {
                    return;
                }
                if let Some(pointer) = &mut state.pointer {
                    pointer.position = Some((surface_x, surface_y));
                    // 进来时不设的话光标是什么样子由合成器定，可能还是应用里的文字光标
                    if let Some(shape) = &pointer.shape {
                        shape.set_shape(serial, Shape::Default);
                    }
                }
                state.hover();
            }
            wl_pointer::Event::Leave { .. } => {
                if let Some(pointer) = &mut state.pointer {
                    pointer.position = None;
                    pointer.wheel = Wheel::default();
                }
                state.hover();
            }
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                if let Some(pointer) = &mut state.pointer
                    && pointer.position.is_some()
                {
                    pointer.position = Some((surface_x, surface_y));
                    state.hover();
                }
            }
            wl_pointer::Event::Button {
                button,
                state: WEnum::Value(ButtonState::Pressed),
                ..
            } => state.press(button),
            wl_pointer::Event::Axis {
                axis: WEnum::Value(Axis::VerticalScroll),
                value,
                ..
            } => state.wheel(proxy, |wheel| wheel.axis(value)),
            wl_pointer::Event::AxisDiscrete {
                axis: WEnum::Value(Axis::VerticalScroll),
                discrete,
            } => state.wheel(proxy, |wheel| wheel.discrete(discrete)),
            wl_pointer::Event::AxisValue120 {
                axis: WEnum::Value(Axis::VerticalScroll),
                value120,
            } => state.wheel(proxy, |wheel| wheel.value120(value120)),
            wl_pointer::Event::AxisStop {
                axis: WEnum::Value(Axis::VerticalScroll),
                ..
            } => state.wheel(proxy, Wheel::stop),
            wl_pointer::Event::Frame => state.scrolled(),
            _ => {}
        }
    }
}
