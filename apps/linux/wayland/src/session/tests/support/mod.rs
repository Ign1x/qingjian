//! 会话测试的假 Server、合成器记录器与共用步骤。

mod answer;
mod call;
mod fake_server;
mod out;
mod recorder;

pub use answer::Answer;
pub use call::Call;
pub use fake_server::FakeServer;
pub use out::Out;
pub use recorder::Recorder;

use crate::session::{Field, Input, Key, Session};
use qingjian_core::{Candidate, CandidateKind, CandidateList};
use qingjian_platform::protocol::linux::DisplayIdentity;
use qingjian_platform::protocol::{Frame, KeyOutcome, PreeditKind, PreeditSegment};
use std::time::Instant;

/// 一个会话，已经拿到焦点、接管了键盘、收到了键位表；两边的记录都清空。
pub fn ready() -> (Session<FakeServer>, Recorder, Instant) {
    let mut session = Session::new(FakeServer::default());
    let mut out = Recorder::default();
    let now = Instant::now();
    session.handle(Input::Activate(Field::default()), &mut out, now);
    session.handle(Input::Keymap("xkb_keymap {};".to_owned()), &mut out, now);
    session.server.calls.clear();
    out.log.clear();
    (session, out, now)
}

pub fn identity(revision: u64) -> DisplayIdentity {
    DisplayIdentity {
        generation: 1,
        context: "wayland".to_owned(),
        revision,
    }
}

/// 一次按键：键码随便给，keysym 用 ASCII 或 X11 的 keysym。
pub fn key(code: u32, keysym: u32, pressed: bool) -> Key {
    Key {
        code,
        keysym,
        mask: 0,
        pressed,
        repeats: true,
        time: 0,
        epoch: 0,
    }
}

/// 组句中的一帧：拼音与候选。
pub fn composing(preedit: &str, candidates: &[&str]) -> Frame {
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

pub fn answer(outcome: KeyOutcome, commit: Option<&str>, frame: Frame) -> Answer {
    Answer {
        outcome,
        commit: commit.map(str::to_owned),
        frame,
        english: None,
    }
}

/// 按下再抬起一个字母，Server 吃掉按下、给出 `frame`。
pub fn type_letter(
    session: &mut Session<FakeServer>,
    out: &mut Recorder,
    letter: char,
    frame: Frame,
    now: Instant,
) {
    session
        .server
        .presses
        .push_back(answer(KeyOutcome::Consumed, None, frame));
    let code = letter as u32;
    session.handle(Input::Key(key(code, code, true)), out, now);
    session.handle(Input::Key(key(code, code, false)), out, now);
}

/// 应用处理完我们的更新、commit 了：合成器转来一次 `done`。
pub fn settled(session: &mut Session<FakeServer>, out: &mut Recorder, now: Instant) {
    session.handle(Input::Update(Field::default()), out, now);
}
