//! 某一刻桌面的外观偏好，以及它怎么落到候选窗上。

use qingjian_platform::Appearance;

/// 缺省值就是读不到 portal 时的样子：浅色、照常播动画。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Preference {
    /// 偏好深色；没有偏好与偏好浅色都是 `false`。
    pub dark: bool,

    /// 减少动态效果：过渡直接跳到终点，循环动画停在第一帧。
    pub reduce_motion: bool,
}

impl Preference {
    /// 配置里的外观落成深浅：「跟随系统」看桌面，浅色、深色照设置。
    pub fn dark_for(self, appearance: Appearance) -> bool {
        match appearance {
            Appearance::System => self.dark,
            Appearance::Light => false,
            Appearance::Dark => true,
        }
    }

    /// 偏好从 `before` 变成这样之后，按外观设置 `appearance` 画着的那一帧要不要重画：
    /// 它落成的深浅变了，或减少动态效果的开关变了（循环动画要停下或接着播）。
    pub fn redraws(self, before: Self, appearance: Appearance) -> bool {
        self.dark_for(appearance) != before.dark_for(appearance)
            || self.reduce_motion != before.reduce_motion
    }
}
