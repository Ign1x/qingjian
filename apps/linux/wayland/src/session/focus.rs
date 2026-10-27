//! 输入框来了、走了、变了。

use super::{Field, Output, Preedit, Server, Session};

impl<S: Server> Session<S> {
    pub(super) fn activate(&mut self, field: Field, out: &mut impl Output) {
        self.active = true;
        // 新输入框里没有我们的拼音，候选窗在上一次失活时已经收起
        self.preedit = Preedit::default();
        self.unsent.clear();
        self.settle_at = None;
        self.shown = None;
        self.poll_at = None;
        if let Err(error) = self.server.capabilities(field.capabilities()) {
            tracing::warn!(%error, "报输入框能力失败");
        }
        self.field = field;
        // 密码框不接管键盘，键直接归应用；之前拿着的键盘由 yield_keyboard 交还
        if !self.field.password {
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
        if !self.field.password && self.server.connected() {
            // 与 Fcitx5 插件同一个顺序。失焦后已经不能往原来的框里上屏，组句由 Server 丢弃
            let _ = self.server.focus(false);
            let _ = self.server.deactivate();
        }
        self.hide(out);
        // 输入框失活时应用自己清掉 preedit；失活之后发的更新合成器不再转给它，攒着的也就作废了
        self.preedit = Preedit::default();
        self.unsent.clear();
        self.settle_at = None;
    }

    pub(super) fn update(&mut self, field: Field, out: &mut impl Output, now: std::time::Instant) {
        if !self.active {
            return;
        }
        // 应用 commit 了：上一次发的它已经处理完，攒着的可以发了
        self.settle_at = None;
        self.flush(out, now);
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
        let moved = field.external_change
            && field.surrounding != self.field.surrounding
            && self.composing();
        self.field = field;
        if moved && self.serving() {
            // 组句中应用自己挪了光标或改了文字（点到别处、应用重置了输入法）：组句已经对不上，丢掉
            tracing::debug!("组句中输入框被应用改了，丢掉组句");
            let epoch = out.epoch();
            match self.server.reset() {
                Ok(reply) => self.apply(reply, epoch, out, now),
                Err(error) => self.lost(&error, out),
            }
        }
    }
}
