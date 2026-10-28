//! 选区翻译：有选区时按下报能读选区，Server 要选区就回选中的文字，之后的回包像按键一样落地。

use qingjian_core::{Candidate, CandidateKind, CandidateList};
use qingjian_platform::protocol::{Frame, KeyOutcome};

use crate::session::tests::support::{Call, FakeServer, answer};
use crate::session::{Field, Surrounding};

/// T 的 evdev 键码；keysym 给大写的 T（按着 Shift），Server 收到的虚拟键码也是它。
pub(super) const T: u32 = 20;

pub(super) const T_KEYSYM: u32 = 'T' as u32;

/// 「我喜欢苹果。」里选中「苹果」（9 到 15 字节），光标在选区后头。
pub(super) fn selecting() -> Field {
    field("我喜欢苹果。", 15, 9)
}

pub(super) fn field(text: &str, cursor: usize, anchor: usize) -> Field {
    Field {
        surrounding: Some(Surrounding {
            text: text.to_owned(),
            cursor,
            anchor,
        }),
        ..Field::default()
    }
}

/// 只有一条候选、没有拼音的帧，与 Server 翻译时给的一样。
pub(super) fn single(text: &str) -> Frame {
    Frame {
        candidates: CandidateList {
            items: vec![Candidate {
                text: text.to_owned(),
                kind: CandidateKind::Cloud,
                syllables: Vec::new(),
                reading: None,
                translation: None,
                aux_code: None,
            }],
        },
        page_count: 1,
        ..Frame::default()
    }
}

/// 按下翻译快捷键，Server 要选区、回了选区就开始翻译：候选窗是「翻译中…」，回包说还在等译文。
pub(super) fn start_translating(server: &mut FakeServer) {
    server.selection_request = Some(7);
    server.pending = true;
    server
        .presses
        .push_back(answer(KeyOutcome::Consumed, None, single("翻译中…")));
}

/// 按下时报的「能读选区」，按先后。
pub(super) fn reported(server: &FakeServer) -> Vec<bool> {
    server
        .calls
        .iter()
        .filter_map(|call| match call {
            Call::Key {
                release: false,
                selection,
                ..
            } => Some(*selection),
            _ => None,
        })
        .collect()
}
