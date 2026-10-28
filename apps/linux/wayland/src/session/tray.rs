//! 托盘上点的切换：发给活动窗口那个应用的会话，先问它现在的模式，不是目标才切。
//! 规则与理由见 `docs/design/linux-wayland.md`「托盘」。

use std::time::Instant;

use qingjian_platform::protocol::linux::client::Reply;

use super::Session;
use crate::session::{ModeTarget, Output, Server};

impl<S: Server> Session<S> {
    /// 输入框激活着就用它正用着的会话（组句中切中 / 英时原文照常上屏）；没有输入框时用活动窗口那个应用的会话——
    /// 没有活动窗口（如点击状态栏后失焦）时保留最近用过的会话。只改模式，不碰应用里的东西。
    pub(super) fn tray(&mut self, target: ModeTarget, out: &mut impl Output, now: Instant) {
        if !self.active && self.app.is_some() {
            self.server.select(self.app.as_deref());
        }
        let epoch = out.epoch();
        let current = match self.server.poll() {
            Ok(reply) => {
                let mode = reply.mode.clone();
                self.landed(reply, epoch, out, now);
                mode
            }
            Err(error) => {
                self.lost(&error, out);
                return;
            }
        };
        // 查询期间发生失焦 / 激活时，先处理排队的焦点事件，不能拿新代次把旧会话的上屏送给新输入框。
        if out.epoch() != epoch {
            return;
        }
        let Some(command) = target.command(current.as_ref()) else {
            return;
        };
        tracing::debug!(?command, "托盘：切换");
        match self.server.indicator(command) {
            Ok(reply) => self.landed(reply, epoch, out, now),
            Err(error) => self.lost(&error, out),
        }
    }

    /// 托盘那条路上的回包：输入框激活着就照常落地；没有输入框时只记下模式（托盘跟着换），上屏与候选都没处去。
    fn landed(&mut self, reply: Reply, epoch: u64, out: &mut impl Output, now: Instant) {
        if self.active {
            self.apply(reply, epoch, out, now);
        } else {
            self.adopt(reply.mode.as_ref(), out);
        }
    }
}
