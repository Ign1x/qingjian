//! 输入框来了、走了、变了；活动窗口换了应用时换 Server 上的会话。

use std::time::Instant;

use super::{FOLLOW, Field, Output, Preedit, Server, Session};

impl<S: Server> Session<S> {
    pub(super) fn activate(&mut self, field: Field, out: &mut impl Output) {
        self.active = true;
        // 新输入框里没有我们的拼音，候选窗在上一次失活时已经收起
        self.preedit = Preedit::default();
        self.unsent.clear();
        self.settle_at = None;
        self.shown = None;
        self.poll_at = None;
        self.follow_at = None;
        self.field = field;
        self.enter(out);
    }

    /// Server 那一侧的进场：选活动窗口那个应用的会话、报能力，不是密码框就报焦点、接管键盘
    /// （密码框不接管，键直接归应用；之前拿着的键盘由 yield_keyboard 交还）。
    fn enter(&mut self, out: &mut impl Output) {
        self.server.select(self.app.as_deref());
        if let Err(error) = self.server.capabilities(self.field.capabilities()) {
            tracing::warn!(%error, "报输入框能力失败");
        }
        if self.field.password {
            tracing::debug!("密码框：不报焦点、不接管键盘，键直接归应用");
        } else {
            self.connect(out);
        }
    }

    /// 告诉 Server 焦点到了，成功就把键盘接过来。连不上就先不接管，过一会儿 tick 再试。
    pub(super) fn connect(&mut self, out: &mut impl Output) {
        match self.server.focus(true) {
            Ok(_) => {
                if !self.grabbed {
                    out.grab(true);
                    self.grabbed = true;
                }
            }
            // Server 没起时每次退避到点都会走到这里，不用 warn 刷屏；断开的那一下 Link 已经记过
            Err(error) => tracing::debug!(%error, "连不上 Server，这个输入框先不接管键盘"),
        }
    }

    pub(super) fn deactivate(&mut self, out: &mut impl Output) {
        if !self.active {
            return;
        }
        self.active = false;
        self.repeat.stop();
        self.poll_at = None;
        self.follow_at = None;
        self.leave();
        self.hide(out);
        // 输入框失活时应用自己清掉 preedit；失活之后发的更新合成器不再转给它，攒着的也就作废了
        self.preedit = Preedit::default();
        self.unsent.clear();
        self.settle_at = None;
    }

    /// Server 那一侧的退场，与 Fcitx5 插件同一个顺序。失焦后已经不能往原来的框里上屏，组句由 Server 丢弃。
    fn leave(&mut self) {
        if !self.field.password && self.server.connected() {
            let _ = self.server.focus(false);
            let _ = self.server.deactivate();
        }
    }

    /// 活动窗口换了应用。没有输入框激活着时记下就行，下次激活用它；激活着（合成器先报了激活、后报活动窗口，
    /// 或应用改了 app_id）就等 [`FOLLOW`] 再跟过去。
    pub(super) fn switch_app(&mut self, app: Option<String>, now: Instant) {
        self.app = app;
        if self.active {
            self.follow_at = Some(now + FOLLOW);
        }
    }

    /// 输入框还激活着，Server 上用着的却不是活动窗口那个应用的会话：换过去，键盘、行内拼音与候选窗都不动。
    /// 组句中先不换（换了这次组句就丢了），过一会儿再看。
    pub(super) fn follow(&mut self, out: &mut impl Output, now: Instant) {
        if !self.active || self.server.app() == self.app.as_deref() {
            return;
        }
        if self.composing() {
            self.follow_at = Some(now + FOLLOW);
            return;
        }
        tracing::debug!("输入框激活期间活动窗口换了应用，换到它的会话");
        self.poll_at = None;
        self.leave();
        self.enter(out);
    }

    pub(super) fn update(&mut self, field: Field, out: &mut impl Output, now: Instant) {
        if !self.active {
            return;
        }
        let invalidated = field.invalidates(&self.field);
        self.settle_at = None;
        if invalidated {
            // 新位置不能接收旧位置排队的文字；隐私能力变化时也不能先上屏再清理。
            self.unsent.clear();
            self.repeat.stop();
        } else {
            // 应用处理完上一次更新，攒着的可以发了。
            self.flush(out, now);
        }
        if field.password != self.field.password {
            // 密码框点了「显示密码」这类：当成换了一个输入框
            self.deactivate(out);
            self.activate(field, out);
            return;
        }
        if field.sensitive != self.field.sensitive {
            // 能力变了 Server 丢弃组句，这边跟着清掉
            if let Err(error) = self.server.capabilities(field.capabilities()) {
                tracing::warn!(%error, "报输入框能力失败");
            }
            self.field = field;
            self.clear(out);
            return;
        }
        self.field = field;
        if invalidated && self.serving() {
            // 回包可能因目标已变化而未显示，本地没有组句也要取消 Server 上的翻译。
            tracing::debug!("输入框被应用改了，重置组句与翻译");
            let epoch = out.epoch();
            match self.server.reset() {
                Ok(reply) => self.apply(reply, epoch, out, now),
                Err(error) => self.lost(&error, out),
            }
        }
    }
}
