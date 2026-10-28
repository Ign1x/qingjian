//! 假 Server 对一次按下的答复。

use qingjian_core::{Candidate, CandidateKind, CandidateList};
use qingjian_platform::protocol::{Frame, KeyOutcome, PreeditKind, PreeditSegment};

#[derive(Debug, Clone)]
pub struct Answer {
    pub outcome: KeyOutcome,

    pub commit: Option<String>,

    /// 换上的帧；`None` 是帧不变。
    pub frame: Option<Frame>,
}

impl Answer {
    /// 吃掉，组句中：拼音与这一页候选。
    pub fn composing(preedit: &str, candidates: &[&str]) -> Self {
        Self {
            outcome: KeyOutcome::Consumed,
            commit: None,
            frame: Some(composing(preedit, candidates)),
        }
    }

    /// 吃掉，上屏 `text`，组句结束（空帧）。
    pub fn commit(text: &str) -> Self {
        Self {
            outcome: KeyOutcome::Consumed,
            commit: Some(text.to_owned()),
            frame: Some(Frame::default()),
        }
    }

    /// 放行给应用，帧不变。
    pub fn passthrough() -> Self {
        Self {
            outcome: KeyOutcome::Passthrough,
            commit: None,
            frame: None,
        }
    }

    /// 换上的帧是第 `page` 页（从 0 起），一共 `count` 页。
    pub fn on_page(mut self, page: usize, count: usize) -> Self {
        if let Some(frame) = &mut self.frame {
            frame.page = page;
            frame.page_count = count;
        }
        self
    }
}

/// 组句中的一帧，与真 Server 发的一样：行内拼音放进应用（缺省的 `preedit_mode`），光标在末尾。
fn composing(preedit: &str, candidates: &[&str]) -> Frame {
    Frame {
        preedit: vec![PreeditSegment {
            text: preedit.to_owned(),
            kind: PreeditKind::Typed,
        }],
        cursor: preedit.chars().count(),
        candidates: CandidateList {
            items: candidates
                .iter()
                .map(|text| Candidate {
                    text: (*text).to_owned(),
                    kind: CandidateKind::Chinese,
                    syllables: Vec::new(),
                    reading: None,
                    translation: None,
                    aux_code: None,
                })
                .collect(),
        },
        page_count: 1,
        ..Frame::default()
    }
}
