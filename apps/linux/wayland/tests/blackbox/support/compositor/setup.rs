//! 假合成器提供哪些全局对象、怎么回话。

#[derive(Debug, Clone, Copy)]
pub struct Setup {
    /// 提供 zwp_input_method_manager_v2（KWin、GNOME 没有）。
    pub input_method: bool,

    /// 拿输入法对象时回 unavailable：同一个 seat 上已经有别的输入法（fcitx5 的 waylandim 开着时就是这样）。
    pub unavailable: bool,

    /// 候选窗 surface 的分数缩放（120 分之几，180 是 1.5 倍），建 wp_fractional_scale_v1 时就发；`None` 是不发。
    pub scale: Option<u32>,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            input_method: true,
            unavailable: false,
            scale: None,
        }
    }
}
