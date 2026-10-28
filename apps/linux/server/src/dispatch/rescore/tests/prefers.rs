//! 测试用选词器：保留打分调用的上下文长度，强烈偏爱「你它」。
use qingjian_core::ChoiceModel;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default)]
pub(super) struct Prefers {
    pub context_chars: AtomicUsize,
}

impl ChoiceModel for Prefers {
    fn align(&self, keys: &str, syllables: &[String]) -> Option<Vec<u32>> {
        (keys == syllables.concat()).then(|| self.letter_units(keys))
    }

    fn letter_units(&self, keys: &str) -> Vec<u32> {
        keys.chars().map(u32::from).collect()
    }

    fn score(&self, context: &str, _typed: &[u32], texts: &[&str]) -> Vec<f32> {
        self.context_chars
            .store(context.chars().count(), Ordering::SeqCst);
        texts
            .iter()
            .map(|text| if *text == "你它" { 100.0 } else { 0.0 })
            .collect()
    }
}
