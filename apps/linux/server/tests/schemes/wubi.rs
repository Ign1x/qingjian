//! 随包的 86 五笔：单用时的满码唯一自动上屏、顶字、标点上屏、空码清除，与拼音同开时的混输；回包报方案名。

use serde_json::Value;

use super::{compose, key, scheme, started, texts, user_dir};

const ESCAPE: u32 = 0x1B;

fn escape(router: &mut qingjian_linux_server::Router) {
    let event = qingjian_platform::protocol::KeyEvent::new(ESCAPE, None, Default::default());
    router.handle_linux(serde_json::json!({"LinuxEvent": {"session": 1, "event": {"Key": {"event": event, "release": false}}}}));
}

#[test]
fn wubi_alone_follows_its_own_rules() {
    let dir = user_dir("wubi");
    let (mut router, _) = started(&dir, "[general]\nscheme = \"none\"\ncode = \"wubi86\"\n");
    let typed = compose(&mut router, "ggll");
    assert_eq!(texts(&typed)[0], "一");
    assert_eq!(scheme(&typed), "五笔（86）");
    assert_eq!(key(&mut router, ' ')["commit"], "一");

    // 满码且只对应一个词：敲满就上屏
    assert_eq!(compose(&mut router, "trwu")["commit"], "我们");

    // 满码后再敲字母：先把高亮候选顶上屏，字母当下一个词的第一码
    assert!(texts(&compose(&mut router, "khlg")).contains(&"中国".to_owned()));
    let topped = key(&mut router, 't');
    assert_eq!(topped["commit"], "中国");
    assert_eq!(topped["frame"]["preedit"][0]["text"], "t");
    escape(&mut router);

    // 标点先上屏高亮候选，再接全角标点（不看 punct_commits）
    compose(&mut router, "khlg");
    assert_eq!(key(&mut router, ',')["commit"], "中国，");

    // 空码：空格只清掉编码，不把字母打进正文
    assert!(texts(&compose(&mut router, "zz")).is_empty());
    let cleared = key(&mut router, ' ');
    assert_eq!(cleared["commit"], Value::Null);
    assert!(cleared["frame"]["preedit"].as_array().unwrap().is_empty());
    drop(router);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn wubi_and_pinyin_mix() {
    let dir = user_dir("mixed");
    let (mut router, _) = started(&dir, "[general]\ncode = \"wubi86\"\n");
    let pinyin = compose(&mut router, "nihao");
    assert!(texts(&pinyin).contains(&"你好".to_owned()), "{pinyin}");
    assert_eq!(scheme(&pinyin), "五笔（86） + 全拼");
    escape(&mut router);
    assert_eq!(texts(&compose(&mut router, "ggll"))[0], "一");
    drop(router);
    std::fs::remove_dir_all(&dir).unwrap();
}
