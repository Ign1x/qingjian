//! 前端报来的一次按键，连同它带的前后文与「能读选区」，测试核对隐私与选区翻译用。

use qingjian_platform::protocol::KeyEvent;
use qingjian_platform::protocol::linux::AppSurrounding;

#[derive(Debug, Clone)]
pub struct KeyReport {
    pub event: KeyEvent,

    pub release: bool,

    /// 光标两侧的文字；敏感框、抬键与应用不报前后文时是 `None`。
    pub surrounding: Option<AppSurrounding>,

    pub selection_supported: bool,
}
