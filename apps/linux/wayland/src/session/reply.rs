//! Server 的回包怎么落地：上屏与行内拼音、候选窗、展示回执、轮询。

use std::time::Instant;

use qingjian_platform::protocol::linux::client::{ClientError, Reply};

use super::preedit::chunks;
use super::{Output, POLL_INTERVAL, Preedit, SETTLE, Server, Session};

/// 一次展示回执最多带几条义项：多了 Server 断开连接。
const MAX_SENSES: usize = 128;

impl<S: Server> Session<S> {
    /// 落地一次回包。`epoch` 是发请求时的输入目标代次：等回包期间焦点、隐私或文字位置变了，上屏与候选都不落地——
    /// 合成器（wlroots、Hyprland）不按 commit 的 serial 丢弃过期请求，文字会落进新的输入框。
    pub(super) fn apply(&mut self, reply: Reply, epoch: u64, out: &mut impl Output, now: Instant) {
        out.sync();
        if !self.active || out.epoch() != epoch {
            self.unsent.clear();
            tracing::debug!("等回包期间输入目标变了，这次的上屏与候选不落地");
            return;
        }
        self.text(
            reply.commit.as_deref(),
            Preedit::from_frame(&reply.frame),
            out,
            now,
        );
        let remode = self.adopt(reply.mode.as_ref(), out);
        self.present(&reply, remode, out);
        if let Some(english) = reply.english {
            self.flash(english, out, now);
        }
        // 右键菜单开着时不轮询（见 `mouse`），菜单收起时再接上
        self.poll_at = (self.menu.is_none()
            && (!reply.frame.is_empty() || reply.translation_pending))
            .then(|| now + POLL_INTERVAL);
    }

    /// 上屏与行内拼音。都没变就不发：每发一次应用都要处理一遍 preedit 与 done。
    /// 上一次发出的还没等到应用落定就先攒着，落定（收到 `done`）或等满 [`super::SETTLE`] 再一起发。
    pub(super) fn text(
        &mut self,
        commit: Option<&str>,
        preedit: Preedit,
        out: &mut impl Output,
        now: Instant,
    ) {
        let commit = commit.unwrap_or_default();
        if !self.unsent.dirty && commit.is_empty() && preedit == self.preedit {
            return;
        }
        self.preedit = preedit;
        self.unsent.push(commit);
        if self.settle_at.is_none() {
            self.flush(out, now);
        }
    }

    /// 把攒着的文字更新发出去。每次 commit 都带上完整的 preedit：合成器在 commit 之后把没设的 preedit 当成空串，不带就闪掉。
    pub(super) fn flush(&mut self, out: &mut impl Output, now: Instant) {
        let Some(commit) = self.unsent.take() else {
            return;
        };
        let parts = chunks(&commit);
        match parts.split_last() {
            Some((last, rest)) => {
                for part in rest {
                    out.text(part, &Preedit::default());
                }
                out.text(last, &self.preedit);
            }
            None => out.text("", &self.preedit),
        }
        self.settle_at = Some(now + SETTLE);
    }

    /// 候选窗：空帧收起；版本号没变（轮询回来帧没变）不理；帧内容没变（抬键的回包）不重画，
    /// 但回执照发——展示身份换了，Server 认的是新身份，点选与右键菜单也带它。`remode` 是模式变了：帧没变也重画。
    /// 中 / 英提示亮着时空帧不收它（切换键是组合键时，抬起另几个键的回包也是空帧），有了候选就换成候选。
    fn present(&mut self, reply: &Reply, remode: bool, out: &mut impl Output) {
        if reply.frame.is_empty() {
            if self.drawn.is_some() {
                self.hide(out);
            }
            self.shown = None;
            return;
        }
        let revision = reply.revision();
        if let (Some(revision), Some(shown)) = (revision, self.shown)
            && revision <= shown
            && !remode
        {
            return;
        }
        if remode
            || self
                .drawn
                .as_ref()
                .is_none_or(|(frame, _)| *frame != reply.frame)
        {
            // 换了一帧：右键菜单对不上了，随新帧一并收起
            self.menu = None;
            let senses = out.show(&reply.frame);
            self.drawn = Some((reply.frame.clone(), senses));
            self.hint_until = None;
        }
        self.shown = revision;
        self.identity.clone_from(&reply.identity);
        if let (Some(identity), Some((_, senses))) = (reply.identity.clone(), &self.drawn) {
            let senses = senses.iter().copied().take(MAX_SENSES).collect();
            if let Err(error) = self.server.acknowledge(identity, senses) {
                tracing::warn!(%error, "展示回执没送出去");
            }
        }
    }

    /// 收起候选窗，连同亮着的中 / 英提示。
    pub(super) fn hide(&mut self, out: &mut impl Output) {
        let hinted = self.hint_until.take().is_some();
        if self.drawn.take().is_some() || hinted {
            out.hide();
        }
        self.shown = None;
        self.identity = None;
        self.menu = None;
    }

    pub(super) fn poll(&mut self, out: &mut impl Output, now: Instant) {
        let epoch = out.epoch();
        match self.server.poll() {
            Ok(reply) => self.apply(reply, epoch, out, now),
            Err(error) => self.lost(&error, out),
        }
    }

    /// 清掉应用里的拼音、收起候选窗、停止轮询（Server 那边的组句已经丢了）。
    pub(super) fn clear(&mut self, out: &mut impl Output) {
        self.text(None, Preedit::default(), out, Instant::now());
        self.hide(out);
        self.poll_at = None;
    }

    /// 与 Server 的往返失败：组句作废，之后的键原样交给应用，键盘松开后交还，过一会儿再连。
    pub(super) fn lost(&mut self, error: &ClientError, out: &mut impl Output) {
        tracing::warn!(%error, "与 Server 的往返失败，组句作废，键先原样交给应用");
        self.repeat.stop();
        self.clear(out);
        self.forget_mode(out);
    }
}
