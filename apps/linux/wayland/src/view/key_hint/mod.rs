//! 键位提示（`[general] key_hint`）：Server 在回包上给的键盘，画进候选窗那一个 popup 里。
//! 摆在哪在 [`place()`]，怎么画在 `paint`，两侧的取舍在 `side`。

mod paint;
mod place;
mod side;

pub use place::place;
pub use side::Side;
