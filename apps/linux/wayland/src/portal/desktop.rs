//! 桌面的外观偏好，跟着 portal 变：后台线程写，画候选窗时读。

use std::sync::atomic::{AtomicBool, Ordering};

use super::{Preference, Setting};

/// 每个开关一个原子量，读写都不加锁。偶尔读到一项新一项旧也无妨：下一秒再比一次就对上了。
#[derive(Debug, Default)]
pub struct Desktop {
    /// `color-scheme` 偏好深色。
    dark: AtomicBool,

    /// `reduced-motion` 为 1。
    reduced_motion: AtomicBool,

    /// GNOME 的 `enable-animations` 关着。
    animations_off: AtomicBool,
}

impl Desktop {
    /// 现在的偏好：几次原子读，每画一帧取一次。
    pub fn preference(&self) -> Preference {
        Preference {
            dark: self.dark.load(Ordering::Relaxed),
            reduce_motion: self.reduced_motion.load(Ordering::Relaxed)
                || self.animations_off.load(Ordering::Relaxed),
        }
    }

    /// 记下 portal 给的一项，返回它变没变。
    pub fn apply(&self, setting: Setting) -> bool {
        let (flag, on) = match setting {
            Setting::ColorScheme { dark } => (&self.dark, dark),
            Setting::ReducedMotion(reduced) => (&self.reduced_motion, reduced),
            Setting::Animations(enabled) => (&self.animations_off, !enabled),
        };
        flag.swap(on, Ordering::Relaxed) != on
    }
}
