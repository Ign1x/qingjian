//! 假 Server 对一次按下的答复。

use qingjian_core::{Candidate, CandidateKind, CandidateList};
use qingjian_platform::protocol::linux::InputMode;
use qingjian_platform::protocol::{Frame, KeyOutcome, PreeditKind, PreeditSegment};

#[derive(Debug, Clone)]
pub struct Answer {
    pub outcome: KeyOutcome,

    pub commit: Option<String>,

    /// 换上的帧；`None` 是帧不变。
    pub frame: Option<Frame>,

    /// 要选区（选区翻译快捷键）：回 `RequestSelection`，带这个请求编号；这一键的结果等前端回了选区再定，
    /// 照真 Server 的规矩：回的是空串（或超过 500 字）就放行，不然吃掉、开始翻译。
    pub select: Option<u64>,

    pub english: Option<bool>,

    pub mode: Option<InputMode>,
}

impl Answer {
    /// 吃掉，组句中：拼音与这一页候选。
    pub fn composing(preedit: &str, candidates: &[&str]) -> Self {
        Self {
            outcome: KeyOutcome::Consumed,
            commit: None,
            frame: Some(composing(preedit, candidates)),
            select: None,
            english: None,
            mode: None,
        }
    }

    /// 吃掉，上屏 `text`，组句结束（空帧）。
    pub fn commit(text: &str) -> Self {
        Self {
            outcome: KeyOutcome::Consumed,
            commit: Some(text.to_owned()),
            frame: Some(Frame::default()),
            select: None,
            english: None,
            mode: None,
        }
    }

    /// 放行给应用，帧不变。
    pub fn passthrough() -> Self {
        Self {
            outcome: KeyOutcome::Passthrough,
            commit: None,
            frame: None,
            select: None,
            english: None,
            mode: None,
        }
    }

    /// 这一键是选区翻译快捷键：先要选区。
    pub fn select(request: u64) -> Self {
        Self {
            select: Some(request),
            ..Self::passthrough()
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

    /// 切换的回包同时带新模式与 `english`，之后的回包只带模式。
    pub fn switched(mut self, mode: InputMode) -> Self {
        self.english = Some(mode.english);
        self.mode = Some(mode);
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
        candidates: list(candidates, CandidateKind::Chinese),
        page_count: 1,
        ..Frame::default()
    }
}

/// 选区翻译的一帧，与真 Server 的 `translation_frame` 一样：没有拼音，只有一条云端候选（「翻译中…」或译文）。
pub fn translation(text: &str) -> Frame {
    Frame {
        candidates: list(&[text], CandidateKind::Cloud),
        page_count: 1,
        ..Frame::default()
    }
}

fn list(candidates: &[&str], kind: CandidateKind) -> CandidateList {
    CandidateList {
        items: candidates
            .iter()
            .map(|text| Candidate {
                text: (*text).to_owned(),
                kind,
                syllables: Vec::new(),
                reading: None,
                translation: None,
                aux_code: None,
            })
            .collect(),
    }
}
