//! 换会话不串：一个输入框里码段敲到一半，切到另一个输入框从拼音态开始，切回来码段原样还在。

use super::{code_segment, compose, key, preedit, router, texts};

#[test]
fn the_code_stays_in_its_own_input_box() {
    let mut router = router();
    let first = compose(&mut router, 1, "kaifa;k");
    assert_eq!(code_segment(&first).as_deref(), Some("k"));

    let other = compose(&mut router, 2, "kaifa");
    assert_eq!(preedit(&other), "kai'fa");
    assert!(
        texts(&other).contains(&"开放".to_owned()),
        "另一个输入框的候选不被筛：{other}"
    );
    let entered = key(&mut router, 2, ';');
    assert_eq!(preedit(&entered), "kai'fa;", "另一个输入框照样能进辅码");
    assert_eq!(code_segment(&entered), None);

    let back = key(&mut router, 1, 'f');
    assert_eq!(preedit(&back), "kai'fa;kf");
    assert_eq!(texts(&back)[0], "开发");
}
