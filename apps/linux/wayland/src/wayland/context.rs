//! [`Output`] 的真实现：会话的决定变成 input-method-v2、虚拟键盘与候选窗上的请求。

use qingjian_platform::protocol::linux::{InputMode, KeyHintView};
use wayland_client::EventQueue;
use wayland_client::backend::WaylandError;

use crate::session::{Field, Modifiers, Output, Preedit};
use crate::wayland::state::Wayland;

/// 处理一件事期间借来的 Wayland 一侧：状态与事件队列（`sync` 要读新事件）。
pub struct Context<'a> {
    pub wayland: &'a mut Wayland,

    pub queue: &'a mut EventQueue<Wayland>,
}

impl Output for Context<'_> {
    fn epoch(&self) -> u64 {
        self.wayland.method.epoch
    }

    fn selection_current(&self, field: &Field) -> bool {
        self.wayland.method.selection_current(field)
    }

    fn sync(&mut self) {
        if let Err(error) = self.queue.flush() {
            tracing::debug!(%error, "送请求失败");
        }
        if let Some(guard) = self.queue.prepare_read() {
            match guard.read() {
                Ok(_) => {}
                Err(WaylandError::Io(error)) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => tracing::debug!(%error, "读合成器事件失败"),
            }
        }
        if let Err(error) = self.queue.dispatch_pending(self.wayland) {
            tracing::debug!(%error, "处理合成器事件失败");
        }
    }

    fn grab(&mut self, on: bool) {
        let wayland = &mut *self.wayland;
        if on {
            if wayland.grab.is_none() {
                tracing::debug!("接管键盘");
                wayland.grab = Some(wayland.input_method.grab_keyboard(&wayland.qh, ()));
            }
        } else if let Some(grab) = wayland.grab.take() {
            tracing::debug!("交还键盘");
            grab.release();
        }
    }

    fn keymap(&mut self, keymap: &str) {
        self.wayland.virtual_keyboard.keymap(keymap);
    }

    fn forward(&mut self, code: u32, pressed: bool, time: u32) {
        self.wayland.virtual_keyboard.key(time, code, pressed);
    }

    fn modifiers(&mut self, modifiers: Modifiers) {
        self.wayland.virtual_keyboard.modifiers(modifiers);
    }

    fn text(&mut self, commit: &str, preedit: &Preedit) {
        let input_method = &self.wayland.input_method;
        if !commit.is_empty() {
            input_method.commit_string(commit.to_owned());
        }
        input_method.set_preedit_string(preedit.text.clone(), preedit.cursor, preedit.cursor);
        input_method.commit(self.wayland.method.dones);
    }

    fn show(&mut self, frame: &qingjian_platform::protocol::Frame) -> Vec<(usize, usize)> {
        self.wayland.show(frame)
    }

    fn hide(&mut self) {
        self.wayland.hide();
    }

    fn open_menu(&mut self, items: &[String]) -> bool {
        self.wayland.open_menu(items)
    }

    fn close_menu(&mut self) {
        self.wayland.close_menu();
    }

    fn mode(&mut self, mode: Option<&InputMode>) {
        self.wayland.set_mode(mode);
    }

    fn hint(&mut self, mode: &InputMode) -> bool {
        self.wayland.hint(mode)
    }

    fn key_hint(&mut self, hint: Option<&KeyHintView>) {
        self.wayland.set_key_hint(hint);
    }
}
