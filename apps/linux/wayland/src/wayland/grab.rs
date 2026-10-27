//! 键盘 grab 的事件：键位表、按键、修饰键、重复设置，算好 keysym 后按到达顺序排队。

use std::fs::File;
use std::os::fd::OwnedFd;

use memmap2::MmapOptions;
use wayland_client::protocol::wl_keyboard::{KeyState, KeymapFormat};
use wayland_client::{Connection, Dispatch, QueueHandle, WEnum};
use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_keyboard_grab_v2::{
    self, ZwpInputMethodKeyboardGrabV2,
};

use super::Wayland;
use super::xkb::Xkb;
use crate::session::{Input, Key, Modifiers};

impl Dispatch<ZwpInputMethodKeyboardGrabV2, ()> for Wayland {
    fn event(
        state: &mut Self,
        _: &ZwpInputMethodKeyboardGrabV2,
        event: zwp_input_method_keyboard_grab_v2::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwp_input_method_keyboard_grab_v2::Event::Keymap { format, fd, size } => {
                if format != WEnum::Value(KeymapFormat::XkbV1) {
                    // 刚 grab 时 seat 上还没有活动键盘，合成器先发一个 no_keymap，等有键盘按键时再发真的
                    tracing::debug!(?format, "键位表还没有，或格式不认识");
                    return;
                }
                match read_keymap(fd, size) {
                    Ok(text) => {
                        state.xkb = Xkb::new(&text);
                        if state.xkb.is_none() {
                            tracing::warn!("键位表编译不了，按键只能原样转给应用");
                        }
                        state.inbox.push_back(Input::Keymap(text));
                    }
                    Err(error) => tracing::warn!(%error, "读不了键位表"),
                }
            }
            zwp_input_method_keyboard_grab_v2::Event::Key {
                time,
                key,
                state: key_state,
                ..
            } => {
                let pressed = match key_state {
                    WEnum::Value(KeyState::Pressed) => true,
                    WEnum::Value(KeyState::Released) => false,
                    // 合成器自己的重复（wl_keyboard v10）不理：吃掉的键我们自己重复，转出去的应用自己重复
                    _ => return,
                };
                // 键位表编译不了时 keysym 给 0：Server 认不出就放行，键原样到应用
                let (keysym, mask, repeats) = state.xkb.as_ref().map_or((0, 0, true), |xkb| {
                    (xkb.keysym(key), xkb.mask(), xkb.repeats(key))
                });
                state.inbox.push_back(Input::Key(Key {
                    code: key,
                    keysym,
                    mask,
                    pressed,
                    repeats,
                    time,
                    epoch: state.method.epoch,
                }));
            }
            zwp_input_method_keyboard_grab_v2::Event::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
                ..
            } => {
                if let Some(xkb) = &mut state.xkb {
                    xkb.update(mods_depressed, mods_latched, mods_locked, group);
                }
                state.inbox.push_back(Input::Modifiers(Modifiers {
                    depressed: mods_depressed,
                    latched: mods_latched,
                    locked: mods_locked,
                    group,
                }));
            }
            zwp_input_method_keyboard_grab_v2::Event::RepeatInfo { rate, delay } => {
                state.inbox.push_back(Input::Repeat { rate, delay });
            }
            _ => {}
        }
    }
}

/// 读合成器给的键位表。wl_keyboard 自 v7 起要求接收方用 MAP_PRIVATE 映射，MAP_SHARED 可能失败。
fn read_keymap(fd: OwnedFd, size: u32) -> std::io::Result<String> {
    let file = File::from(fd);
    // SAFETY: 只读的私有映射，合成器不会再改这块内存
    let map = unsafe {
        MmapOptions::new()
            .len(size as usize)
            .map_copy_read_only(&file)?
    };
    let end = map.iter().position(|byte| *byte == 0).unwrap_or(map.len());
    Ok(String::from_utf8_lossy(&map[..end]).into_owned())
}
