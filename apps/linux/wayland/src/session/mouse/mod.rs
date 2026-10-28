//! 鼠标在候选窗上：左键点候选上屏、滚轮翻页、右键候选弹菜单、点菜单里的一项。
//!
//! 点中了什么由 `crate::wayland` 按画出来的那一张认好（[`Spot`]），这里只管问 Server、回包怎么落地——与按键的回包同一条路
//! （[`Session::apply`]：等应用落定再发、焦点换了就丢）。请求带候选窗上那一帧的展示身份，帧已经换了 Server 回 `Ignored`，当没点过。
//! 菜单开着时停掉轮询：轮询换了帧，展示身份跟着变，再点菜单里的项就被 Server 当过期的丢掉。

mod menu;

pub use menu::Menu;

use std::time::Instant;

use qingjian_platform::protocol::linux::client::{ClientError, Reply};

use crate::session::{Click, Output, POLL_INTERVAL, Server, Session, Spot};

impl<S: Server> Session<S> {
    pub(super) fn click(&mut self, click: Click, out: &mut impl Output, now: Instant) {
        // 候选窗开着、键交给 Server 处理时才算数：候选窗收起之后才排到的点击不理
        if !self.serving() || self.drawn.is_none() || out.epoch() != click.epoch {
            return;
        }
        if self.menu.is_some() {
            match click.spot {
                Some(Spot::MenuItem(item)) => self.pick(item, click.epoch, out, now),
                Some(Spot::Menu) => {}
                Some(Spot::Candidate(index)) if click.right => {
                    self.close_menu(out, now);
                    self.open_menu(index, click.epoch, out);
                }
                // 点到菜单外面只收起菜单、不选词，与系统的右键菜单一样
                _ => self.close_menu(out, now),
            }
            return;
        }
        match click.spot {
            Some(Spot::Candidate(index)) if click.right => self.open_menu(index, click.epoch, out),
            Some(Spot::Candidate(index)) => {
                if let Some(identity) = self.identity.clone() {
                    let reply = self.server.candidate(identity, index);
                    self.answer(reply, click.epoch, out, now);
                }
            }
            _ => {}
        }
    }

    /// 滚轮翻一页。只有一页时不问 Server；菜单开着时不翻（翻了菜单就对不上右键的那一格了）。
    pub(super) fn scroll(&mut self, next: bool, epoch: u64, out: &mut impl Output, now: Instant) {
        if !self.serving() || self.menu.is_some() || out.epoch() != epoch {
            return;
        }
        let pages = self.drawn.as_ref().map_or(0, |(frame, _)| frame.page_count);
        if pages < 2 {
            return;
        }
        if let Some(identity) = self.identity.clone() {
            let reply = self.server.page(identity, next);
            self.answer(reply, epoch, out, now);
        }
    }

    /// 收起右键菜单（按了键、点到菜单外面），接着轮询。
    pub(super) fn close_menu(&mut self, out: &mut impl Output, now: Instant) {
        if self.menu.take().is_some() {
            out.close_menu();
            self.resume_polling(now);
        }
    }

    /// 右键本页第 `index` 个候选：问 Server 这一格能做哪几件事，有就弹菜单、停掉轮询。
    fn open_menu(&mut self, index: usize, epoch: u64, out: &mut impl Output) {
        let Some(identity) = self.identity.clone() else {
            return;
        };
        let items = match self.server.menu(identity, index) {
            Ok(items) => items,
            Err(error) => {
                self.lost(&error, out);
                return;
            }
        };
        out.sync();
        if out.epoch() != epoch || items.is_empty() {
            return;
        }
        let labels: Vec<String> = items.iter().map(|item| item.label.clone()).collect();
        if out.open_menu(&labels) {
            let actions = items.iter().map(|item| item.id).collect();
            self.menu = Some(Menu::new(index, actions));
            self.poll_at = None;
        }
    }

    /// 点了菜单里的第 `item` 项：收起菜单，让 Server 去做；回包的帧带着做完的那句提示。
    fn pick(&mut self, item: usize, epoch: u64, out: &mut impl Output, now: Instant) {
        let Some(menu) = self.menu.take() else {
            return;
        };
        out.close_menu();
        if let (Some(action), Some(identity)) = (menu.action(item), self.identity.clone()) {
            let reply = self.server.menu_pick(identity, menu.index, action);
            self.answer(reply, epoch, out, now);
        }
        self.resume_polling(now);
    }

    /// 点选、翻页、菜单项的回包。帧已经换了的，Server 回 `Ignored`，客户端读成不带展示身份的缺省答复
    /// （真的回包都带身份）：什么都不做，不然空帧会把候选窗收起来。
    fn answer(
        &mut self,
        reply: Result<Reply, ClientError>,
        epoch: u64,
        out: &mut impl Output,
        now: Instant,
    ) {
        match reply {
            Ok(reply) if reply.identity.is_none() => {
                tracing::debug!("候选窗上那一帧已经换了，这一下 Server 没理");
            }
            Ok(reply) => self.apply(reply, epoch, out, now),
            Err(error) => self.lost(&error, out),
        }
    }

    /// 菜单收起之后接着轮询：候选窗还开着、回包没另排的话。
    fn resume_polling(&mut self, now: Instant) {
        if self.poll_at.is_none() && self.drawn.is_some() && self.menu.is_none() {
            self.poll_at = Some(now + POLL_INTERVAL);
        }
    }
}
