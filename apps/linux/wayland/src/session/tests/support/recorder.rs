//! 会话测试的合成器记录器。

use super::Out;
use crate::session::{Field, Modifiers, Output, Preedit};
use qingjian_platform::protocol::Frame;

/// 记录输出的假合成器。`sync` 时可以假装焦点换了。
#[derive(Default)]
pub struct Recorder {
    pub log: Vec<Out>,

    pub epoch: u64,

    /// 下一次 `sync` 时焦点代次跳一下（等回包期间用户点到了别处）。
    pub switch_on_sync: bool,
}

impl Output for Recorder {
    fn epoch(&self) -> u64 {
        self.epoch
    }

    fn selection_current(&self, _field: &Field) -> bool {
        true
    }

    fn sync(&mut self) {
        if std::mem::take(&mut self.switch_on_sync) {
            self.epoch += 2;
        }
    }

    fn grab(&mut self, on: bool) {
        self.log.push(Out::Grab(on));
    }

    fn keymap(&mut self, _keymap: &str) {
        self.log.push(Out::Keymap);
    }

    fn forward(&mut self, code: u32, pressed: bool, _time: u32) {
        self.log.push(Out::Forward(code, pressed));
    }

    fn modifiers(&mut self, modifiers: Modifiers) {
        self.log.push(Out::Modifiers(modifiers.depressed));
    }

    fn text(&mut self, commit: &str, preedit: &Preedit) {
        self.log
            .push(Out::Text(commit.to_owned(), preedit.text.clone()));
    }

    fn show(&mut self, frame: &Frame) -> Vec<(usize, usize)> {
        let texts = frame
            .candidates
            .items
            .iter()
            .map(|candidate| candidate.text.clone())
            .collect();
        self.log.push(Out::Show(texts));
        vec![(0, 0)]
    }

    fn hide(&mut self) {
        self.log.push(Out::Hide);
    }
}
