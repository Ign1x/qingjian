//! 候选窗挂上的一块缓冲。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Picture {
    /// 缓冲的像素宽高。
    pub width: i32,

    pub height: i32,

    /// 有不是全透明的像素：渲染器真的画了东西。
    pub painted: bool,
}
