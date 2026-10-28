//! 选区翻译：应用里有选区时按键报「能读选区」，Server 要选区（翻译快捷键）就从最近一次前后文里取出选中的文字回过去。
//! 来由见 `docs/design/linux-wayland.md`「协议对照」。

use qingjian_platform::protocol::linux::client::{ClientError, Reply};
use qingjian_platform::protocol::linux::selected_text;

use super::{Server, Session};

impl<S: Server> Session<S> {
    /// 这一键报不报 `selection_supported`：应用报的前后文里有选区（cursor 与 anchor 不等）。
    /// 敏感输入框不带前后文，也不读选区；密码框的键根本不报 Server。
    pub(super) fn selectable(&self) -> bool {
        !self.field.sensitive
            && !self.field.password
            && self
                .field
                .surrounding
                .as_ref()
                .is_some_and(|surrounding| surrounding.cursor != surrounding.anchor)
    }

    /// Server 要选区：从最近一次前后文里取出选中的文字回过去，那一键的结果是这次的回包。
    /// 取不出来（偏移不对、超过 500 字）或是敏感输入框时回空串，Server 放行，快捷键原样交给应用。
    pub(super) fn answer_selection(&mut self, request: u64) -> Result<Reply, ClientError> {
        let text = self
            .field
            .surrounding
            .as_ref()
            .filter(|_| self.selectable())
            .and_then(|surrounding| {
                selected_text(&surrounding.text, surrounding.cursor, surrounding.anchor)
            })
            .unwrap_or_default()
            .to_owned();
        // 日志不记选中的文字，只记读没读到
        tracing::debug!(read = !text.is_empty(), "Server 要应用里选中的文字");
        self.server.selection(request, text)
    }
}
