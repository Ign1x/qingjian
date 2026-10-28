//! 输入模式：回包上的 `mode` 记下来交给显示面（主题的 `mode.*`），变了才交；切中 / 英的那一下在候选窗的位置亮一下提示。

use std::time::Instant;

use qingjian_platform::protocol::linux::InputMode;

use super::{HINT, Output, Server, Session};

impl<S: Server> Session<S> {
    /// 记下回包上的模式（老 Server 不带时不动），变了才告诉显示面；返回变没变。
    /// 模式是 Server 上当前那个应用的会话的，换应用、切中 / 英、托盘改了标点或简繁都会变。
    pub(super) fn adopt(&mut self, mode: Option<&InputMode>, out: &mut impl Output) -> bool {
        let Some(mode) = mode else {
            return false;
        };
        if self.mode.as_ref() == Some(mode) {
            return false;
        }
        self.mode = Some(mode.clone());
        out.mode(Some(mode));
        true
    }

    /// 与 Server 的往返断了：Server 那边的会话连同模式都没了，重连之后从缺省算起，这边先当不知道。
    pub(super) fn forget_mode(&mut self, out: &mut impl Output) {
        if self.mode.take().is_some() {
            out.mode(None);
        }
    }

    /// 刚切了中 / 英（`english` 是切到哪边）：候选窗空着就在它的位置亮一下现在的模式，[`HINT`] 之后收起。
    /// 候选窗开着时不盖住它（切中 / 英时组句原样上屏、候选窗本来就收了，开着是少见的情形）。
    /// 只在输入框激活时调：popup 只有那时才显示。
    pub(super) fn flash(&mut self, english: bool, out: &mut impl Output, now: Instant) {
        if self.drawn.is_some() {
            return;
        }
        let mode = match &self.mode {
            Some(mode) if mode.english == english => mode.clone(),
            // 老 Server 回包不带模式：只知道切到了哪边
            _ => InputMode {
                english,
                ..InputMode::default()
            },
        };
        if out.hint(&mode) {
            self.hint_until = Some(now + HINT);
        }
    }

    /// 提示到点：收起。中间画上了候选的话 `hint_until` 早已清掉，不会走到这里。
    pub(super) fn unflash(&mut self, out: &mut impl Output) {
        self.hint_until = None;
        if self.drawn.is_none() {
            out.hide();
        }
    }
}
