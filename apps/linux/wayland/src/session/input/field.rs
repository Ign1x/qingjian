//! 一个输入框当前报给输入法的状态。

use qingjian_platform::protocol::linux::Capabilities;

use super::Surrounding;

/// input-method-v2 在 `done` 时整份生效的那几样，折成输入法关心的意思。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Field {
    /// 密码框、PIN、隐藏文字：输入法不碰，键盘直接归应用。
    pub password: bool,

    /// 敏感内容：照常组句，但不学习、不带前后文、不发云端。
    pub sensitive: bool,

    /// 光标附近的文字；应用不报就是 `None`。
    pub surrounding: Option<Surrounding>,

    /// 这次前后文变化是应用自己改的（光标被挪走、文字被改），不是输入法上屏引起的。
    pub external_change: bool,
}

impl Field {
    /// 报给 Server 的能力。
    pub fn capabilities(&self) -> Capabilities {
        Capabilities {
            sensitive: self.sensitive,
            password: self.password,
            disabled: false,
        }
    }
}
