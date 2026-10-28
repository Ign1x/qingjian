//! 小键盘数字（`[general] numpad_digit`）：缺省选候选，「直接输入」时普通中文组句里按输入顺序进缓冲区，其他模式不变。
use super::support::{compose, key, router};
use crate::dispatch::Router;
use qingjian_platform::NumpadDigit;
use qingjian_platform::protocol::{Frame, KeyModifiers, KeyOutcome};

/// 小键盘数字 `n`：共用键表把 KP_0..KP_9 报成 0x60..0x69，字符是数字本身。
fn keypad(
    router: &mut Router,
    n: u32,
    modifiers: KeyModifiers,
) -> (KeyOutcome, Option<String>, Frame) {
    key(router, 0x60 + n, char::from_digit(n, 10), modifiers)
}

fn direct(size: usize) -> Router {
    let mut router = router(size);
    router.config.numpad_digit = NumpadDigit::Direct;
    router
}

#[test]
fn default_keypad_selects_current_page_candidate() {
    let mut router = router(5);
    compose(&mut router, "qq", KeyModifiers::default());
    let (outcome, commit, frame) = keypad(&mut router, 2, KeyModifiers::default());
    assert_eq!(
        (outcome, commit.as_deref()),
        (KeyOutcome::Consumed, Some("第2项"))
    );
    assert!(frame.candidates.items.is_empty());
}

/// 直接输入：小键盘数字接在敲的字母后面，`Space` / `Enter` 时按输入顺序原样上屏。
#[test]
fn direct_keypad_digits_keep_input_order_until_space_or_enter() {
    let normal = KeyModifiers::default();
    for n in 0..=9 {
        for (code, character, suffix) in [(0x20, Some(' '), " "), (0x0D, None, "")] {
            let mut router = direct(5);
            compose(&mut router, "qq", normal);
            let (outcome, commit, _) = keypad(&mut router, n, normal);
            assert_eq!((outcome, commit), (KeyOutcome::Consumed, None));
            assert_eq!(router.engine.composition().text(), format!("qq{n}"));
            compose(&mut router, "gpt", normal);
            let (_, commit, frame) = key(&mut router, code, character, normal);
            assert_eq!(commit, Some(format!("qq{n}gpt{suffix}")));
            assert!(frame.candidates.items.is_empty());
        }
    }
}

/// 直接输入只改小键盘：主键盘数字照旧选当前页候选。
#[test]
fn direct_does_not_change_main_digits() {
    let mut router = direct(5);
    compose(&mut router, "qq", KeyModifiers::default());
    assert_eq!(
        key(&mut router, 0x32, Some('2'), KeyModifiers::default())
            .1
            .as_deref(),
        Some("第2项")
    );
}

/// 选候选时这一页没有第 N 个：小键盘数字与主键盘一样进缓冲区。
#[test]
fn keypad_without_a_candidate_slot_enters_buffer_in_select_mode() {
    let mut router = router(1);
    compose(&mut router, "ni", KeyModifiers::default());
    let (_, commit, _) = keypad(&mut router, 9, KeyModifiers::default());
    assert_eq!(commit, None);
    assert_eq!(router.engine.composition().text(), "ni9");
}

/// 英文候选、Caps、表达式与带 Win 的组合保持原语义。
#[test]
fn english_caps_expression_and_command_keys_keep_their_semantics() {
    let mut router = direct(5);
    let english = KeyModifiers {
        english_mode: true,
        ..Default::default()
    };
    compose(&mut router, "hel", english);
    let first = router.current_frame().candidates.items[0].text.clone();
    assert_eq!(keypad(&mut router, 1, english).1, Some(first));
    let caps = KeyModifiers {
        caps: true,
        ..Default::default()
    };
    assert_eq!(keypad(&mut router, 5, caps).0, KeyOutcome::Passthrough);

    let normal = KeyModifiers::default();
    compose(&mut router, "v", normal);
    keypad(&mut router, 1, normal);
    key(&mut router, 0x6B, Some('+'), normal);
    let frame = keypad(&mut router, 2, normal).2;
    assert!(frame.candidates.items.iter().any(|c| c.text == "3"));
    let win = KeyModifiers {
        win: true,
        ..Default::default()
    };
    let (outcome, commit, _) = keypad(&mut router, 5, win);
    assert_eq!((outcome, commit), (KeyOutcome::Passthrough, None));
    assert_eq!(router.engine.composition().text(), "v1+2");
}

/// 码点输入（`u4e00`）照样收小键盘数字；问字里的小键盘数字与选候选时一样，不进缓冲区。
#[test]
fn unicode_entry_takes_keypad_digits_and_question_mode_keeps_its_digits() {
    let normal = KeyModifiers::default();
    let mut router = direct(5);
    compose(&mut router, "u", normal);
    keypad(&mut router, 4, normal);
    compose(&mut router, "e", normal);
    keypad(&mut router, 0, normal);
    let frame = keypad(&mut router, 0, normal).2;
    assert_eq!(frame.candidates.items[0].text, "一");

    let mut asking = direct(5);
    let mut selecting = super::support::router(5);
    for router in [&mut asking, &mut selecting] {
        compose(router, "uni", normal);
        assert!(router.engine.question_mode());
    }
    let asked = keypad(&mut asking, 1, normal);
    let selected = keypad(&mut selecting, 1, normal);
    assert_eq!((asked.0, asked.1), (selected.0, selected.1));
    assert_eq!(
        asking.engine.composition().text(),
        selecting.engine.composition().text()
    );
}
