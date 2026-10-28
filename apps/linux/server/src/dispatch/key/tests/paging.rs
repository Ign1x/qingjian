//! 翻页键：额外翻页键（`[shortcut] extra_page_keys`）与 `-` `=` 翻不动时落回原字符。
use super::support::{compose, key, router};
use qingjian_platform::protocol::{KeyModifiers, KeyOutcome};

#[test]
fn all_extra_pairs_page_chinese_candidates_without_committing() {
    let mut baseline = router(1);
    let mut router = router(1);
    router.config.extra_page_keys = vec!["[]".into(), ",.".into(), "-=".into()]
        .try_into()
        .unwrap();
    let normal = KeyModifiers::default();
    compose(&mut router, "qq", normal);
    for (previous, next) in [('[', ']'), (',', '.'), ('-', '=')] {
        let forward = key(&mut router, next as u32, Some(next), normal);
        assert_eq!(
            (forward.0, forward.1, forward.2.page),
            (KeyOutcome::Consumed, None, 1)
        );
        let back = key(&mut router, previous as u32, Some(previous), normal);
        assert_eq!(
            (back.0, back.1, back.2.page),
            (KeyOutcome::Consumed, None, 0)
        );
    }
    // 清掉额外键后，逗号恢复为原有输入行为
    router.config.extra_page_keys = Default::default();
    compose(&mut baseline, "qq", normal);
    let expected = key(&mut baseline, b',' as u32, Some(','), normal);
    let actual = key(&mut router, b',' as u32, Some(','), normal);
    assert_eq!((actual.0, actual.1), (expected.0, expected.1));
    assert_eq!(router.engine.composition(), baseline.engine.composition());
}

/// 英文候选里 `-` `=` 作翻页键：`-` 翻得回上一页；第一页没得翻时 `-` 仍是单词里的连字符。
#[test]
fn english_minus_pages_back_and_stays_a_hyphen_on_the_first_page() {
    let mut router = router(1);
    router.config.page_keys = ('-', '=');
    let english = KeyModifiers {
        english_mode: true,
        ..Default::default()
    };
    compose(&mut router, "hel", english);
    assert_eq!(key(&mut router, b'=' as u32, Some('='), english).2.page, 1);
    let back = key(&mut router, b'-' as u32, Some('-'), english);
    assert_eq!(
        (back.0, back.1, back.2.page),
        (KeyOutcome::Consumed, None, 0)
    );
    assert_eq!(router.engine.composition().text(), "hel");
    key(&mut router, b'-' as u32, Some('-'), english);
    assert_eq!(router.engine.composition().text(), "hel-");
}

/// 中文组句里 `-` `=` 翻不动时落回原字符进直输段；`[` `]` 翻不动照旧吃键。
#[test]
fn chinese_minus_equals_fall_back_to_characters_at_page_edges() {
    let normal = KeyModifiers::default();
    // `qq` 九个候选、每页四个：三页
    let composing = |page_keys: (char, char)| {
        let mut router = router(4);
        router.config.page_keys = page_keys;
        compose(&mut router, "qq", normal);
        router
    };
    let mut first = composing(('-', '='));
    key(&mut first, b'-' as u32, Some('-'), normal);
    assert_eq!(first.engine.composition().text(), "qq-");

    let mut last = composing(('-', '='));
    for page in 1..=2 {
        assert_eq!(key(&mut last, b'=' as u32, Some('='), normal).2.page, page);
    }
    assert_eq!(key(&mut last, b'-' as u32, Some('-'), normal).2.page, 1);
    key(&mut last, b'=' as u32, Some('='), normal);
    key(&mut last, b'=' as u32, Some('='), normal);
    assert_eq!(last.engine.composition().text(), "qq=");

    let mut brackets = composing(('[', ']'));
    let result = key(&mut brackets, b'[' as u32, Some('['), normal);
    assert_eq!(
        (result.0, result.1, result.2.page),
        (KeyOutcome::Consumed, None, 0)
    );
    assert_eq!(brackets.engine.composition().text(), "qq");
}
