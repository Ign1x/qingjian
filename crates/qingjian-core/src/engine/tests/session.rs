//! 换出换入输入上下文（`swap_session`）：一个输入框里组句到一半的东西不串到另一个，丢弃输入时一起清掉。

use std::sync::Arc;

use qingjian_dictionary::{AuxCodeLookup, AuxCodeTable};

use super::engine;
use crate::{CandidateKind, Engine, EngineSession};

fn aux_engine() -> Engine {
    let table: Arc<dyn AuxCodeLookup> = Arc::new(
        AuxCodeTable::from_pairs([
            ("开发".to_owned(), "kf".to_owned()),
            ("开".to_owned(), "kh".to_owned()),
        ])
        .unwrap(),
    );
    let mut engine = engine().with_aux_codes(vec![table]);
    engine.set_aux_enabled(true);
    engine
}

fn candidates(engine: &Engine) -> Vec<String> {
    engine
        .query()
        .unwrap()
        .candidates
        .items
        .into_iter()
        .map(|c| c.text)
        .collect()
}

fn pick(engine: &mut Engine, text: &str) -> String {
    let candidate = engine
        .query()
        .unwrap()
        .candidates
        .items
        .into_iter()
        .find(|c| c.text == text && c.kind == CandidateKind::Chinese)
        .unwrap();
    engine.commit(&candidate)
}

/// 码段敲到一半换到别的输入框：那边从拼音态开始、候选不被筛，换回来码段原样还在。
#[test]
fn aux_code_stays_with_its_session() {
    let mut engine = aux_engine();
    engine.set_input("kaifa");
    assert!(engine.aux_trigger(';'));
    engine.enter_aux();
    assert!(engine.push_aux_code('k'));
    let mut other = EngineSession::default();
    engine.swap_session(&mut other);

    assert!(!engine.in_aux());
    engine.set_input("kaifa");
    assert!(engine.aux_trigger(';'), "这个输入框还没进辅码");
    assert!(!engine.push_aux_code('k'), "字母不该进别人的码段");
    assert!(candidates(&engine).contains(&"开放".to_owned()));

    engine.swap_session(&mut other);
    assert!(engine.in_aux());
    assert_eq!(engine.aux_code(), "k");
    assert_eq!(engine.composition().text(), "kaifa");
    let filtered = candidates(&engine);
    assert!(filtered.contains(&"开发".to_owned()));
    assert!(!filtered.contains(&"开放".to_owned()));
}

/// 分几次选词时先选的词留在原来的输入框：别的输入框上屏时不带上它，换回来照样接在前面。
#[test]
fn held_selections_stay_with_their_session() {
    let mut engine = engine();
    engine.set_input("kaifaxian");
    assert_eq!(pick(&mut engine, "开发"), "");
    let mut other = EngineSession::default();
    engine.swap_session(&mut other);

    assert_eq!(engine.held_text(), "");
    engine.set_input("xian");
    assert_eq!(pick(&mut engine, "先"), "先");

    engine.swap_session(&mut other);
    assert_eq!(engine.held_text(), "开发");
    assert_eq!(engine.composition().text(), "xian");
    assert_eq!(pick(&mut engine, "先"), "开发先");
}

/// 隐私边界丢弃输入：先选好的词与码段也不留，不管是装着的还是挂起的。
#[test]
fn discarding_input_drops_held_words_and_the_aux_code() {
    let mut engine = aux_engine();
    let compose = |engine: &mut Engine| {
        engine.set_input("kaifaxian");
        pick(engine, "开发");
        assert!(engine.aux_trigger(';'));
        engine.enter_aux();
        engine.push_aux_code('x');
    };
    compose(&mut engine);
    engine.discard_input();
    assert_eq!(engine.held_text(), "");
    assert!(!engine.in_aux());
    assert!(engine.composition().is_empty());

    compose(&mut engine);
    let mut saved = EngineSession::default();
    engine.swap_session(&mut saved);
    saved.discard_input();
    engine.swap_session(&mut saved);
    assert_eq!(engine.held_text(), "");
    assert!(!engine.in_aux());
    assert!(engine.composition().is_empty());
}
