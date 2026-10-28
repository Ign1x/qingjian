//! 全角标点与简繁的切换键（`[shortcut] toggle_punctuation` / `toggle_traditional`）：组句与否都认，简繁连组句中的候选一起换。
use super::support::{compose, key, router};
use crate::dispatch::{Router, RouterConfig};
use qingjian_core::{Engine, TraditionalVariant};
use qingjian_dictionary::Dictionary;
use qingjian_platform::protocol::{
    ClientMessage, KeyModifiers, KeyOutcome, PROTOCOL_VERSION, SessionId,
};
use qingjian_platform::{Config, KeyCombo};

const CTRL_SHIFT: KeyModifiers = KeyModifiers {
    ctrl: true,
    shift: true,
    alt: false,
    win: false,
    caps: false,
    english_mode: false,
};

#[test]
fn punctuation_key_flips_the_current_modes_width_and_keeps_the_composition() {
    let mut router = router(5);
    router.config.toggle_punctuation = Some("ctrl+.".parse().unwrap());
    let normal = KeyModifiers::default();
    let ctrl = KeyModifiers {
        ctrl: true,
        ..normal
    };
    let (outcome, commit, _) = key(&mut router, b'.' as u32, Some('.'), ctrl);
    assert_eq!((outcome, commit), (KeyOutcome::Consumed, None));
    assert!(!router.config.full_width);
    assert_eq!(
        key(&mut router, b',' as u32, Some(','), normal).0,
        KeyOutcome::Passthrough,
        "半角时逗号交给应用"
    );

    compose(&mut router, "ni", normal);
    let (outcome, commit, frame) = key(&mut router, b'.' as u32, Some('.'), ctrl);
    assert_eq!((outcome, commit), (KeyOutcome::Consumed, None));
    assert!(router.config.full_width);
    assert_eq!(router.engine.composition().text(), "ni");
    assert!(!frame.candidates.items.is_empty());

    // 没配切换键：Ctrl + . 归应用
    router.config.toggle_punctuation = None;
    key(&mut router, 0x1B, None, normal);
    assert_eq!(
        key(&mut router, b'.' as u32, Some('.'), ctrl).0,
        KeyOutcome::Passthrough
    );
}

#[test]
fn traditional_key_preserves_composition_and_changes_commit() {
    let engine = Engine::new(Dictionary::parse("汉语\than yu\t100\n").unwrap());
    let mut router = Router::new(
        engine,
        RouterConfig {
            toggle_traditional: Some(KeyCombo::TRADITIONAL_DEFAULT),
            ..Default::default()
        },
    );
    router.handle(ClientMessage::OpenSession {
        session: SessionId(1),
        app: None,
        protocol: PROTOCOL_VERSION,
    });
    let normal = KeyModifiers::default();
    compose(&mut router, "hanyu", normal);
    // Caps 亮着、或 Shift 让字母变大写都一样认
    let chord = KeyModifiers {
        caps: true,
        ..CTRL_SHIFT
    };
    let (outcome, commit, frame) = key(&mut router, b'F' as u32, Some('F'), chord);
    assert_eq!((outcome, commit), (KeyOutcome::Consumed, None));
    assert_eq!(router.engine.composition().text(), "hanyu");
    assert!(frame.candidates.items.iter().any(|c| c.text == "漢語"));
    assert_eq!(
        key(&mut router, 0x20, Some(' '), normal).1.as_deref(),
        Some("漢語")
    );
    key(&mut router, b'f' as u32, Some('f'), CTRL_SHIFT);
    compose(&mut router, "hanyu", normal);
    assert_eq!(
        key(&mut router, 0x20, Some(' '), normal).1.as_deref(),
        Some("汉语")
    );
}

#[test]
fn disabled_or_rebound_traditional_key_leaves_the_combination_to_the_app() {
    let mut router = router(9);
    router.config.toggle_traditional = None;
    assert_eq!(
        key(&mut router, b'F' as u32, Some('F'), CTRL_SHIFT).0,
        KeyOutcome::Passthrough
    );
    router.config.toggle_traditional = Some("ctrl+alt+g".parse().unwrap());
    assert_eq!(
        key(&mut router, b'F' as u32, Some('F'), CTRL_SHIFT).0,
        KeyOutcome::Passthrough
    );
    let rebound = KeyModifiers {
        ctrl: true,
        alt: true,
        english_mode: true,
        ..Default::default()
    };
    assert_eq!(
        key(&mut router, b'g' as u32, Some('g'), rebound).0,
        KeyOutcome::Consumed
    );
    assert_ne!(router.engine.traditional(), TraditionalVariant::Off);

    // 与选区翻译同一个组合时不登记，保留翻译
    let mut config = Config::default();
    config.shortcut.toggle_traditional = Some(config.shortcut.translate_selection);
    assert!(RouterConfig::from(&config).toggle_traditional.is_none());
}

#[test]
fn traditional_key_returns_to_the_variant_used_before_simplified() {
    let mut router = router(9);
    router.config.toggle_traditional = Some(KeyCombo::TRADITIONAL_DEFAULT);
    router.engine.set_traditional(TraditionalVariant::Hongkong);
    key(&mut router, b'F' as u32, Some('F'), CTRL_SHIFT);
    assert_eq!(router.engine.traditional(), TraditionalVariant::Off);
    key(&mut router, b'F' as u32, Some('F'), CTRL_SHIFT);
    assert_eq!(router.engine.traditional(), TraditionalVariant::Hongkong);
}

/// 数字键的组合按数字行认：Shift + 1 的字符是 `!`，键表把键码折回了 0x31。
#[test]
fn digit_combinations_match_the_number_row() {
    let mut router = router(9);
    router.config.toggle_punctuation = Some("ctrl+shift+1".parse().unwrap());
    assert_eq!(
        key(&mut router, 0x31, Some('!'), CTRL_SHIFT).0,
        KeyOutcome::Consumed
    );
    assert!(!router.config.full_width);
}
