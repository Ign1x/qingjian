//! 合成器那边发生的、要 [`super::Session`] 处理的事，按到达顺序排队交过来。

mod field;
mod key;
mod modifiers;
mod surrounding;

pub use field::Field;
pub use key::Key;
pub use modifiers::Modifiers;
pub use surrounding::Surrounding;

/// 一件事。input-method-v2 的状态在 `done` 时整份生效，所以输入框的进出与变化都是 `done` 之后才排进来。
#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    /// 有输入框要输入法了。
    Activate(Field),

    /// 没有输入框要输入法了。
    Deactivate,

    /// 激活着的输入框报了新状态（前后文、内容类型、变化原因）。
    Update(Field),

    /// 键盘布局（xkb 文本格式）：转发按键的虚拟键盘要用同一份。
    Keymap(String),

    /// 一次按下或抬起。
    Key(Key),

    /// 修饰键与布局组的状态。
    Modifiers(Modifiers),

    /// 按住多久开始重复（毫秒）、每秒重复几次；`rate` 为 0 是不重复。
    Repeat { rate: i32, delay: i32 },

    /// 活动窗口换了应用（合成器经 foreign-toplevel 报的 app_id）；没有激活着的窗口、窗口没报 app_id 时是 `None`。
    App(Option<String>),
}
