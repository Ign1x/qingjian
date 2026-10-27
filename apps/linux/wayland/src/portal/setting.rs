//! portal 里青简关心的几项设置，以及从 D-Bus 给的值里认出它们。

use zbus::zvariant::Value;

const APPEARANCE: &str = "org.freedesktop.appearance";

/// 一项设置与它的新值；别的键一概不认。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    /// `color-scheme`：只有 1（偏好深色）算深色，0（无偏好）、2（偏好浅色）与不认识的值都按浅色。
    ColorScheme { dark: bool },

    /// `reduced-motion` 是不是 1（减少动态效果）；0 与不认识的值都不减少。
    ReducedMotion(bool),

    /// GNOME 的 `enable-animations`：关着也算减少动态效果。
    Animations(bool),
}

impl Setting {
    /// 深浅偏好的（命名空间, 键）。
    pub const COLOR_SCHEME: (&'static str, &'static str) = (APPEARANCE, "color-scheme");

    /// 减少动态效果：版本新一些的 portal 才有。
    pub const REDUCED_MOTION: (&'static str, &'static str) = (APPEARANCE, "reduced-motion");

    /// GNOME 的动画开关：GNOME 系的 portal 后端把 `org.gnome.desktop.interface` 整个转出来。
    pub const ENABLE_ANIMATIONS: (&'static str, &'static str) =
        ("org.gnome.desktop.interface", "enable-animations");

    /// 从 `ReadOne` / `Read` 的回复或 `SettingChanged` 认出一项；别的键、类型不对的值为 `None`。
    /// `Read` 的值多包了一层 variant，这里剥掉。
    pub fn parse(namespace: &str, key: &str, value: &Value<'_>) -> Option<Self> {
        match ((namespace, key), innermost(value)) {
            (Self::COLOR_SCHEME, Value::U32(scheme)) => {
                Some(Self::ColorScheme { dark: *scheme == 1 })
            }
            (Self::REDUCED_MOTION, Value::U32(motion)) => Some(Self::ReducedMotion(*motion == 1)),
            (Self::ENABLE_ANIMATIONS, Value::Bool(enabled)) => Some(Self::Animations(*enabled)),
            _ => None,
        }
    }
}

fn innermost<'v, 'a>(value: &'v Value<'a>) -> &'v Value<'a> {
    match value {
        Value::Value(inner) => innermost(inner),
        other => other,
    }
}
