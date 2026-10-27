//! 虚拟键盘：被测进程把放行的键经它转给应用。键位表、按键与修饰键都记下来；
//! 键位表之前就发键或修饰键是协议错误（wlroots 回 no_keymap 并断开），记成违规。

use std::fs::File;
use std::os::fd::OwnedFd;
use std::os::unix::fs::FileExt;
use std::sync::atomic::{AtomicBool, Ordering};

use wayland_protocols_misc::zwp_virtual_keyboard_v1::server::zwp_virtual_keyboard_manager_v1::{
    self, ZwpVirtualKeyboardManagerV1,
};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::server::zwp_virtual_keyboard_v1::{
    self, ZwpVirtualKeyboardV1,
};
use wayland_server::{Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New};

use super::{Request, State};

impl GlobalDispatch<ZwpVirtualKeyboardManagerV1, ()> for State {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        resource: New<ZwpVirtualKeyboardManagerV1>,
        _: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<ZwpVirtualKeyboardManagerV1, ()> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &ZwpVirtualKeyboardManagerV1,
        request: zwp_virtual_keyboard_manager_v1::Request,
        _: &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let zwp_virtual_keyboard_manager_v1::Request::CreateVirtualKeyboard { id, .. } = request
        {
            data_init.init(id, AtomicBool::new(false));
        }
    }
}

/// 用户数据是「收到过键位表」。
impl Dispatch<ZwpVirtualKeyboardV1, AtomicBool> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &ZwpVirtualKeyboardV1,
        request: zwp_virtual_keyboard_v1::Request,
        keymapped: &AtomicBool,
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        let request = match request {
            zwp_virtual_keyboard_v1::Request::Keymap { fd, size, .. } => {
                keymapped.store(true, Ordering::Relaxed);
                Request::Keymap(read_keymap(fd, size))
            }
            zwp_virtual_keyboard_v1::Request::Key { time, key, state } => Request::Key {
                code: key,
                pressed: state == 1,
                time,
            },
            zwp_virtual_keyboard_v1::Request::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
            } => Request::Modifiers {
                depressed: mods_depressed,
                latched: mods_latched,
                locked: mods_locked,
                group,
            },
            _ => return,
        };
        if !matches!(request, Request::Keymap(_)) && !keymapped.load(Ordering::Relaxed) {
            state.requests.push(Request::Violation(format!(
                "虚拟键盘还没有键位表就发了 {request:?}"
            )));
        }
        state.requests.push(request);
    }
}

/// 读虚拟键盘的键位表，到第一个 0 为止。客户端写完没把读写位置挪回开头，所以按偏移读。
fn read_keymap(fd: OwnedFd, size: u32) -> String {
    let file = File::from(fd);
    let mut bytes = vec![0; size as usize];
    if let Err(error) = file.read_exact_at(&mut bytes, 0) {
        return format!("<读不了键位表：{error}>");
    }
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}
