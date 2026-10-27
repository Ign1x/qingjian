//! 一个按着的键按下那次的处置。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Held {
    /// 按下转给了应用：抬起也一定转，不管 Server 对抬起怎么说（不然应用里这个键一直按着）。
    pub forwarded: bool,

    /// 按下报给了 Server：抬起也要报，单击 Shift / Ctrl 切中英认的就是抬起。
    pub reported: bool,
}
