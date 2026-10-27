//! 应用报的光标前后文。

/// 光标附近的一段文字，`cursor` / `anchor` 是 UTF-8 字节偏移（两者不等时中间是选区）。
/// 应用报的文字里不含输入法的 preedit。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Surrounding {
    pub text: String,

    pub cursor: usize,

    pub anchor: usize,
}
