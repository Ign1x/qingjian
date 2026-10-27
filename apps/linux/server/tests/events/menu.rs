//! 候选右键菜单（`Menu` / `MenuPick`）：问这一格能做什么不换帧，点中一项做完带提示重排，过期身份一律 `Ignored`。
use crate::support::{compose, event, key, reply, router_on};
use qingjian_core::{Candidate, Engine, Learner};
use qingjian_dictionary::Dictionary;
use qingjian_linux_server::{Router, RouterConfig};
use qingjian_platform::Config;
use qingjian_platform::candidate_menu::CandidateAction;
use serde_json::{Value, json};
use std::collections::HashMap;

const DOWN: u32 = 0x28;

/// 只记降权的学习数据：引擎缺省的 `NoLearner` 降了权也不挪位置。
#[derive(Default)]
struct Demotions(HashMap<(String, String), u32>);

impl Learner for Demotions {
    fn record(&mut self, _: &Candidate) {}

    fn weight(&self, _: &str) -> u32 {
        0
    }

    fn demote(&mut self, input: &str, text: &str) {
        *self.0.entry((input.into(), text.into())).or_default() += 1;
    }

    fn undemote(&mut self, input: &str, text: &str) -> bool {
        self.0.remove(&(input.into(), text.into())).is_some()
    }

    fn demotion(&self, input: &str, text: &str) -> u32 {
        self.0
            .get(&(input.into(), text.into()))
            .copied()
            .unwrap_or(0)
    }
}

/// 一页三格，`ni` 出「你 泥 拟」。
fn router() -> Router {
    let engine = Engine::new(Dictionary::parse("你\tni\t80\n泥\tni\t70\n拟\tni\t60\n").unwrap())
        .with_learner(Box::new(Demotions::default()));
    router_on(
        engine,
        RouterConfig {
            page_size: 3,
            ..Default::default()
        },
    )
}

fn texts(result: &Value) -> Vec<String> {
    result["frame"]["candidates"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["text"].as_str().unwrap().to_owned())
        .collect()
}

fn labels(menu: &Value) -> Vec<String> {
    menu["Menu"]["items"]
        .as_array()
        .unwrap_or_else(|| panic!("应回菜单：{menu}"))
        .iter()
        .map(|item| item["label"].as_str().unwrap().to_owned())
        .collect()
}

fn menu(router: &mut Router, identity: &Value, index: usize) -> Value {
    reply(
        router,
        1,
        json!({"Menu": {"identity": identity, "index": index}}),
    )
}

fn pick(router: &mut Router, identity: &Value, index: usize, action: CandidateAction) -> Value {
    reply(
        router,
        1,
        json!({"MenuPick": {"identity": identity, "index": index, "action": action.id()}}),
    )
}

#[test]
fn menu_lists_the_slot_and_a_pick_reorders_with_a_notice() {
    let mut router = router();
    let typed = compose(&mut router, 1, "ni");
    assert_eq!(texts(&typed), ["你", "泥", "拟"]);
    let identity = typed["identity"].clone();

    let offered = menu(&mut router, &identity, 0);
    assert_eq!(offered["Menu"]["session"], 1);
    assert_eq!(labels(&offered), ["置顶", "降权", "删除"]);
    assert_eq!(
        offered["Menu"]["items"][1]["id"],
        CandidateAction::Demote.id()
    );

    // 问菜单不推进展示身份：点菜单项带的还是这一帧的身份
    let picked = pick(&mut router, &identity, 0, CandidateAction::Demote)["KeyResult"].clone();
    assert_eq!(picked["outcome"], "Consumed");
    assert_eq!(texts(&picked), ["泥", "你", "拟"]);
    let notice = picked["frame"]["notice"].as_str().expect("做完带提示");
    assert!(notice.contains("你") && notice.contains("降权"), "{notice}");
    assert!(
        picked["identity"]["revision"].as_u64() > identity["revision"].as_u64(),
        "重排之后是新的一帧"
    );

    // 降过权的那一格多出「取消降权」；提示只活到下一次按键
    let again = menu(&mut router, &picked["identity"], 1);
    assert!(labels(&again).contains(&"取消降权".to_owned()), "{again}");
    assert_eq!(
        key(&mut router, 1, DOWN, None, false)["frame"]["notice"],
        json!(null)
    );
}

#[test]
fn stale_or_foreign_menu_events_are_ignored() {
    let mut router = router();
    let old = compose(&mut router, 1, "ni")["identity"].clone();
    let other = reply(
        &mut router,
        2,
        json!({"Menu": {"identity": old, "index": 0}}),
    );
    assert_eq!(other["Ignored"]["session"], 2, "别的会话拿不到这一帧的菜单");

    let moved = key(&mut router, 1, DOWN, None, false);
    assert_ne!(moved["identity"], old);
    assert_eq!(menu(&mut router, &old, 0)["Ignored"]["session"], 1);
    assert_eq!(
        pick(&mut router, &old, 0, CandidateAction::Demote)["Ignored"]["session"],
        1
    );
    let unchanged = key(&mut router, 1, DOWN, None, false);
    assert_eq!(texts(&unchanged), ["你", "泥", "拟"], "过期的点选不能降权");
}

#[test]
fn nothing_to_offer_outside_a_candidate() {
    let mut router = router();
    let identity = compose(&mut router, 1, "ni")["identity"].clone();
    assert!(
        labels(&menu(&mut router, &identity, 5)).is_empty(),
        "页外的格子"
    );
    let unknown = reply(
        &mut router,
        1,
        json!({"MenuPick": {"identity": identity, "index": 0, "action": 99}}),
    )["KeyResult"]
        .clone();
    assert_eq!(unknown["outcome"], "Passthrough");
    assert_eq!(unknown["frame"]["notice"], json!(null));
    assert_eq!(texts(&unknown), ["你", "泥", "拟"]);

    // 上屏之后没在组句：身份还是这个会话的当前帧，但没有候选可问
    let committed = key(&mut router, 1, 0x20, Some(' '), false);
    assert_eq!(committed["commit"], "你");
    assert!(labels(&menu(&mut router, &committed["identity"], 0)).is_empty());
}

#[test]
fn pin_writes_the_phrase_and_puts_it_first() {
    let directory =
        std::env::temp_dir().join(format!("qingjian-linux-menu-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("config.toml");
    std::fs::write(&path, "[general]\n").unwrap();
    let mut router = router();
    router.watch_config(
        &Config::load(&path).unwrap(),
        path.clone(),
        directory.clone(),
        directory.clone(),
    );

    let identity = compose(&mut router, 1, "ni")["identity"].clone();
    assert!(labels(&menu(&mut router, &identity, 1)).contains(&"置顶".to_owned()));
    let pinned = pick(&mut router, &identity, 1, CandidateAction::Pin)["KeyResult"].clone();
    assert_eq!(pinned["outcome"], "Consumed");
    assert_eq!(texts(&pinned)[0], "泥");
    assert!(
        pinned["frame"]["notice"]
            .as_str()
            .is_some_and(|notice| notice.contains("置顶")),
        "{pinned}"
    );
    let phrases = Config::load(&path).unwrap().custom_phrases;
    assert!(
        phrases.iter().any(|p| p.code == "ni" && p.text == "泥"),
        "{phrases:?}"
    );
    assert!(labels(&menu(&mut router, &pinned["identity"], 0)).contains(&"取消置顶".to_owned()));
    assert_eq!(
        event(&mut router, 1, json!("Reset"))["frame"]["notice"],
        json!(null)
    );
    std::fs::remove_dir_all(directory).unwrap();
}

/// 配置文件改到一半存下了坏文件（类型不对的合法 TOML）：不弹菜单、点了也不做——
/// 置顶是读出短语表加一条再整表写回，拿空表当底子会把用户已有的自定义短语全抹掉。
#[test]
fn a_broken_config_never_loses_custom_phrases() {
    let directory =
        std::env::temp_dir().join(format!("qingjian-linux-menu-broken-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("config.toml");
    let good = "[general]\n\n[[custom_phrases]]\ncode = \"wo\"\ntext = \"我的签名\"\n";
    std::fs::write(&path, good).unwrap();
    let mut router = router();
    router.watch_config(
        &Config::load(&path).unwrap(),
        path.clone(),
        directory.clone(),
        directory.clone(),
    );
    let identity = compose(&mut router, 1, "ni")["identity"].clone();
    assert!(labels(&menu(&mut router, &identity, 1)).contains(&"置顶".to_owned()));

    let broken =
        "[general]\npage_size = \"九\"\n\n[[custom_phrases]]\ncode = \"wo\"\ntext = \"我的签名\"\n";
    std::fs::write(&path, broken).unwrap();
    assert!(labels(&menu(&mut router, &identity, 1)).is_empty());
    let picked = pick(&mut router, &identity, 1, CandidateAction::Pin);
    assert_ne!(picked["KeyResult"]["outcome"], "Consumed", "{picked}");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        broken,
        "坏文件原样留着，等用户改好"
    );
    std::fs::remove_dir_all(directory).unwrap();
}
