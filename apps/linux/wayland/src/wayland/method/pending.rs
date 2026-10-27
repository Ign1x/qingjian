//! 下一次 `done` 生效的那一份。

use crate::session::Surrounding;

#[derive(Debug, Default)]
pub struct Pending {
    /// 这一批里最后一个是 activate（`Some(true)`）还是 deactivate（`Some(false)`）。
    pub active: Option<bool>,

    pub surrounding: Option<Surrounding>,

    /// 变化原因是 other（应用自己改的）。
    pub external_change: bool,
}
