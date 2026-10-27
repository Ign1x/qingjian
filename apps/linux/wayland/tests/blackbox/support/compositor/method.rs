//! input-method-v2：输入法对象、候选窗角色与键盘 grab。文字更新、接管与交还键盘都记下来。

use wayland_protocols_misc::zwp_input_method_v2::server::zwp_input_method_keyboard_grab_v2::{
    self, ZwpInputMethodKeyboardGrabV2,
};
use wayland_protocols_misc::zwp_input_method_v2::server::zwp_input_method_manager_v2::{
    self, ZwpInputMethodManagerV2,
};
use wayland_protocols_misc::zwp_input_method_v2::server::zwp_input_method_v2::{
    self, ZwpInputMethodV2,
};
use wayland_protocols_misc::zwp_input_method_v2::server::zwp_input_popup_surface_v2::{
    self, ZwpInputPopupSurfaceV2,
};
use wayland_server::{Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New};

use super::{Request, State};

impl GlobalDispatch<ZwpInputMethodManagerV2, ()> for State {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        resource: New<ZwpInputMethodManagerV2>,
        _: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<ZwpInputMethodManagerV2, ()> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &ZwpInputMethodManagerV2,
        request: zwp_input_method_manager_v2::Request,
        _: &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let zwp_input_method_manager_v2::Request::GetInputMethod { input_method, .. } = request {
            let method = data_init.init(input_method, ());
            if state.setup.unavailable {
                method.unavailable();
            }
            state.input_method = Some(method);
        }
    }
}

impl Dispatch<ZwpInputMethodV2, ()> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &ZwpInputMethodV2,
        request: zwp_input_method_v2::Request,
        _: &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            zwp_input_method_v2::Request::CommitString { text } => {
                state.requests.push(Request::CommitString(text));
            }
            zwp_input_method_v2::Request::SetPreeditString {
                text,
                cursor_begin,
                cursor_end,
            } => state.requests.push(Request::Preedit {
                text,
                begin: cursor_begin,
                end: cursor_end,
            }),
            zwp_input_method_v2::Request::DeleteSurroundingText {
                before_length,
                after_length,
            } => state.requests.push(Request::DeleteSurrounding {
                before: before_length,
                after: after_length,
            }),
            zwp_input_method_v2::Request::Commit { serial } => {
                state.requests.push(Request::Commit(serial));
            }
            zwp_input_method_v2::Request::GetInputPopupSurface { id, surface } => {
                data_init.init(id, ());
                state.popup = Some(surface);
            }
            zwp_input_method_v2::Request::GrabKeyboard { keyboard } => {
                let grab = data_init.init(keyboard, ());
                state.requests.push(Request::Grab);
                state.grabbed(grab);
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwpInputPopupSurfaceV2, ()> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &ZwpInputPopupSurfaceV2,
        _: zwp_input_popup_surface_v2::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
    }
}

impl Dispatch<ZwpInputMethodKeyboardGrabV2, ()> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &ZwpInputMethodKeyboardGrabV2,
        request: zwp_input_method_keyboard_grab_v2::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        if let zwp_input_method_keyboard_grab_v2::Request::Release = request {
            state.requests.push(Request::Release);
            state.grab = None;
        }
    }
}
