//! 虚拟键盘：Server 放行的键经它原样转给应用。
//!
//! 必须与输入法在同一条 Wayland 连接上：合成器只对输入法自己的虚拟键盘不再 grab，不然转出去的键又回到 grab 里。
//! 进程里只建一个、不随焦点重建（每次焦点切换都重建会让合成器反复编译键位表）。

use std::os::fd::AsFd;

use wayland_client::protocol::wl_keyboard::KeymapFormat;
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1;

use super::memfd;
use crate::session::Modifiers;

pub struct VirtualKeyboard {
    proxy: ZwpVirtualKeyboardV1,
}

impl VirtualKeyboard {
    pub fn new(proxy: ZwpVirtualKeyboardV1) -> Self {
        Self { proxy }
    }

    /// 换键位表：内容写进匿名文件，连同结尾的 0 一起交出去。
    pub fn keymap(&self, text: &str) {
        let mut bytes = Vec::with_capacity(text.len() + 1);
        bytes.extend_from_slice(text.as_bytes());
        bytes.push(0);
        match memfd::with_contents(c"qingjian-keymap", &bytes) {
            Ok(file) => {
                self.proxy
                    .keymap(KeymapFormat::XkbV1 as u32, file.as_fd(), bytes.len() as u32)
            }
            Err(error) => tracing::warn!(%error, "给虚拟键盘建键位表失败"),
        }
    }

    pub fn key(&self, time: u32, code: u32, pressed: bool) {
        self.proxy.key(time, code, u32::from(pressed));
    }

    pub fn modifiers(&self, modifiers: Modifiers) {
        self.proxy.modifiers(
            modifiers.depressed,
            modifiers.latched,
            modifiers.locked,
            modifiers.group,
        );
    }
}
