//! 输入模式：回包上的 `mode` 记下来交给显示面（主题的 `mode.*`），变了才交。

use qingjian_platform::protocol::linux::InputMode;

use super::{Output, Server, Session};

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
}
