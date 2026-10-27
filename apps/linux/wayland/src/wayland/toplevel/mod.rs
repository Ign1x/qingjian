//! 活动窗口是哪个应用：zwlr_foreign_toplevel_manager_v1 报每个顶层窗口的 app_id 与状态，`done` 时整份生效；
//! 激活着的那个窗口的应用变了就排一件 [`Input::App`] 给会话。窗口标题也会报，不读。
//!
//! 合成器没有这个全局对象时什么都不报，会话一直按不知道哪个应用处理（所有输入框共用一个会话）。

mod window;

#[cfg(test)]
mod tests;

use std::collections::HashMap;

use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, event_created_child};
use wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_handle_v1::{
    self, ZwlrForeignToplevelHandleV1,
};
use wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_manager_v1::{
    self, ZwlrForeignToplevelManagerV1,
};

use window::Window;

use super::Wayland;
use crate::session::Input;

/// 合成器报的全部顶层窗口，按协议对象编号记。
#[derive(Debug, Default)]
pub struct Toplevels {
    windows: HashMap<u32, Window>,

    /// 激活过几次：几个窗口同时报激活着（多个 seat）时取最后激活的那个。
    activations: u64,

    /// 上次报给会话的应用。
    reported: Option<String>,
}

impl Toplevels {
    /// 窗口报了 app_id，下一次 `done` 生效。
    pub fn app_id(&mut self, id: u32, app_id: String) {
        self.windows.entry(id).or_default().pending_app = Some(app_id);
    }

    /// 窗口报了状态，下一次 `done` 生效。
    pub fn state(&mut self, id: u32, activated: bool) {
        self.windows.entry(id).or_default().pending_activated = Some(activated);
    }

    /// `done`：这个窗口待生效的整份生效。
    pub fn done(&mut self, id: u32) {
        let activations = &mut self.activations;
        self.windows.entry(id).or_default().apply(|| {
            *activations += 1;
            *activations
        });
    }

    pub fn closed(&mut self, id: u32) {
        self.windows.remove(&id);
    }

    /// 活动窗口的应用比上次报的变了，就交出新的（外层的 `None` 是没变）。
    pub fn changed(&mut self) -> Option<Option<String>> {
        let focused = self.focused().map(str::to_owned);
        if focused == self.reported {
            return None;
        }
        self.reported = focused.clone();
        Some(focused)
    }

    /// 激活着的窗口里最后激活的那个的 app_id；没有激活着的窗口、它没报 app_id 或报了空串是 `None`。
    fn focused(&self) -> Option<&str> {
        self.windows
            .values()
            .filter_map(|window| Some((window.activated?, window)))
            .max_by_key(|(order, _)| *order)
            .and_then(|(_, window)| window.app.as_deref())
            .filter(|app| !app.is_empty())
    }
}

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for Wayland {
    fn event(
        _: &mut Self,
        _: &ZwlrForeignToplevelManagerV1,
        event: zwlr_foreign_toplevel_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // 新窗口（toplevel 事件）不用在这里记：它的 app_id、状态与 done 紧跟着就来
        if let zwlr_foreign_toplevel_manager_v1::Event::Finished = event {
            tracing::info!(
                "合成器不再报新开的窗口，焦点落到之后新开的窗口上时按不知道哪个应用处理"
            );
        }
    }

    event_created_child!(Wayland, ZwlrForeignToplevelManagerV1, [
        zwlr_foreign_toplevel_manager_v1::EVT_TOPLEVEL_OPCODE => (ZwlrForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for Wayland {
    fn event(
        state: &mut Self,
        handle: &ZwlrForeignToplevelHandleV1,
        event: zwlr_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let id = handle.id().protocol_id();
        match event {
            zwlr_foreign_toplevel_handle_v1::Event::AppId { app_id } => {
                state.toplevels.app_id(id, app_id);
            }
            zwlr_foreign_toplevel_handle_v1::Event::State { state: states } => {
                state.toplevels.state(id, activated(&states));
            }
            zwlr_foreign_toplevel_handle_v1::Event::Done => {
                state.toplevels.done(id);
                report(state);
            }
            zwlr_foreign_toplevel_handle_v1::Event::Closed => {
                state.toplevels.closed(id);
                handle.destroy();
                report(state);
            }
            // 标题不读（输入法用不上，也不该看）；所在的输出、父窗口用不上
            _ => {}
        }
    }
}

/// 活动窗口的应用变了就排给会话。
fn report(state: &mut Wayland) {
    if let Some(app) = state.toplevels.changed() {
        tracing::debug!(?app, "活动窗口换了应用");
        state.inbox.push_back(Input::App(app));
    }
}

/// `state` 事件的数组是一串本机字节序的 u32，里面有 activated 就是激活着。
fn activated(states: &[u8]) -> bool {
    let activated = zwlr_foreign_toplevel_handle_v1::State::Activated as u32;
    states
        .chunks_exact(4)
        .filter_map(|chunk| <[u8; 4]>::try_from(chunk).ok())
        .any(|bytes| u32::from_ne_bytes(bytes) == activated)
}
