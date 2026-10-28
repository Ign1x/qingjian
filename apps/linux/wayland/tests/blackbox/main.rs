//! Wayland 前端的黑盒测试：真的 `qingjian-linux-wayland` 二进制连测试进程里的假合成器（wayland-server）与假 Server，
//! 走真的 Wayland 协议与 Server 协议。CI 上没有合成器，这是那条路在 CI 上的覆盖；会话规则的细节在 `src/session/tests/`。
//! 怎么搭的、什么情况下跳过，见 `docs/notes/crate-notes.md` 的 apps/linux/wayland 一节。

#![cfg(target_os = "linux")]

mod forwarding;
mod pointer;
mod popup;
mod privacy;
mod selection;
mod startup;
mod support;
mod surrounding;
mod typing;
