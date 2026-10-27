//! portal 给的值怎么认、几个开关怎么合、什么时候要重画。

use qingjian_platform::Appearance;
use zbus::zvariant::Value;

use super::super::{Desktop, Preference, Setting, record};

fn parse((namespace, key): (&str, &str), value: Value<'_>) -> Option<Setting> {
    Setting::parse(namespace, key, &value)
}

/// 只有 1 是深色；0 无偏好、2 偏好浅色、不认识的值都按浅色。
#[test]
fn only_color_scheme_one_is_dark() {
    assert_eq!(
        parse(Setting::COLOR_SCHEME, Value::U32(1)),
        Some(Setting::ColorScheme { dark: true })
    );
    for scheme in [0, 2, 3, u32::MAX] {
        assert_eq!(
            parse(Setting::COLOR_SCHEME, Value::U32(scheme)),
            Some(Setting::ColorScheme { dark: false }),
            "{scheme}"
        );
    }
}

/// `Read` 的值多包一层 variant，剥掉再认；类型不对的值、别的键都不认。
#[test]
fn read_values_are_unwrapped_and_others_ignored() {
    let wrapped = Value::Value(Box::new(Value::U32(1)));
    assert_eq!(
        parse(Setting::COLOR_SCHEME, wrapped.clone()),
        Some(Setting::ColorScheme { dark: true })
    );
    assert_eq!(
        parse(Setting::COLOR_SCHEME, Value::Value(Box::new(wrapped))),
        Some(Setting::ColorScheme { dark: true })
    );
    assert_eq!(parse(Setting::COLOR_SCHEME, Value::I32(1)), None);
    assert_eq!(
        parse(Setting::COLOR_SCHEME, Value::from("prefer-dark")),
        None
    );
    let appearance = Setting::COLOR_SCHEME.0;
    assert_eq!(parse((appearance, "contrast"), Value::U32(1)), None);
    // GNOME 自己那一项是字符串，portal 已经换算成 org.freedesktop.appearance 的那一项
    let gnome = Setting::ENABLE_ANIMATIONS.0;
    assert_eq!(parse((gnome, "color-scheme"), Value::U32(1)), None);
}

#[test]
fn motion_settings_are_recognised() {
    assert_eq!(
        parse(Setting::REDUCED_MOTION, Value::U32(1)),
        Some(Setting::ReducedMotion(true))
    );
    for motion in [0, 2] {
        assert_eq!(
            parse(Setting::REDUCED_MOTION, Value::U32(motion)),
            Some(Setting::ReducedMotion(false)),
            "{motion}"
        );
    }
    assert_eq!(
        parse(Setting::ENABLE_ANIMATIONS, Value::Bool(false)),
        Some(Setting::Animations(false))
    );
    assert_eq!(parse(Setting::ENABLE_ANIMATIONS, Value::U32(0)), None);
    assert_eq!(parse(Setting::REDUCED_MOTION, Value::Bool(true)), None);
}

/// 缺省浅色、照常播动画；只在值变了时报变化；两种减少动态效果任一开着都算。
#[test]
fn the_desktop_records_settings_and_reports_changes() {
    let desktop = Desktop::default();
    assert_eq!(desktop.preference(), Preference::default());
    assert!(!desktop.apply(Setting::ColorScheme { dark: false }));
    assert!(desktop.apply(Setting::ColorScheme { dark: true }));
    assert!(!desktop.apply(Setting::ColorScheme { dark: true }));
    assert_eq!(
        desktop.preference(),
        Preference {
            dark: true,
            reduce_motion: false,
        }
    );
    assert!(desktop.apply(Setting::Animations(false)));
    assert!(desktop.preference().reduce_motion);
    assert!(desktop.apply(Setting::ReducedMotion(true)));
    assert!(desktop.apply(Setting::Animations(true)));
    assert!(desktop.preference().reduce_motion, "reduced-motion 还开着");
    assert!(desktop.apply(Setting::ReducedMotion(false)));
    assert!(!desktop.preference().reduce_motion);
    assert!(desktop.preference().dark, "深浅不受动画开关影响");
}

/// portal 给的一项：认得的、值变了的才返回。
#[test]
fn record_returns_only_changes() {
    let desktop = Desktop::default();
    let (namespace, key) = Setting::COLOR_SCHEME;
    assert_eq!(record(&desktop, namespace, key, &Value::U32(2)), None);
    assert_eq!(
        record(&desktop, namespace, key, &Value::U32(1)),
        Some(Setting::ColorScheme { dark: true })
    );
    assert_eq!(record(&desktop, namespace, key, &Value::U32(1)), None);
    assert_eq!(
        record(&desktop, namespace, "accent-color", &Value::U32(2)),
        None
    );
    assert!(desktop.preference().dark);
    assert_eq!(
        record(&desktop, namespace, key, &Value::U32(0)),
        Some(Setting::ColorScheme { dark: false })
    );
}

#[test]
fn appearance_resolves_against_the_desktop() {
    let light = Preference::default();
    let dark = Preference {
        dark: true,
        ..light
    };
    assert!(!light.dark_for(Appearance::System));
    assert!(dark.dark_for(Appearance::System));
    for preference in [light, dark] {
        assert!(!preference.dark_for(Appearance::Light));
        assert!(preference.dark_for(Appearance::Dark));
    }
}

/// 只有「跟随系统」的帧要跟着桌面换深浅；减少动态效果变了总要重画（循环动画要停下或接着播）；没变不重画。
#[test]
fn redraws_only_when_the_shown_frame_would_change() {
    let light = Preference::default();
    let dark = Preference {
        dark: true,
        ..light
    };
    let still = Preference {
        reduce_motion: true,
        ..light
    };
    assert!(dark.redraws(light, Appearance::System));
    assert!(light.redraws(dark, Appearance::System));
    for fixed in [Appearance::Light, Appearance::Dark] {
        assert!(!dark.redraws(light, fixed), "{fixed:?}");
        assert!(!light.redraws(dark, fixed), "{fixed:?}");
    }
    for appearance in Appearance::ALL {
        assert!(still.redraws(light, appearance), "{appearance:?}");
        assert!(light.redraws(still, appearance), "{appearance:?}");
        for preference in [light, dark, still] {
            assert!(
                !preference.redraws(preference, appearance),
                "{appearance:?}"
            );
        }
    }
}
