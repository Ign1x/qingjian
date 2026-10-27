//! 假 Server 与假合成器。

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use qingjian_core::{Candidate, CandidateKind, CandidateList};
use qingjian_platform::protocol::linux::client::{ClientError, Reply};
use qingjian_platform::protocol::linux::{AppSurrounding, Capabilities, DisplayIdentity};
use qingjian_platform::protocol::{Frame, KeyEvent, KeyOutcome, PreeditKind, PreeditSegment};

use super::super::{Field, Input, Key, Modifiers, Output, Preedit, Server, Session};

/// 假 Server 收到的请求。
#[derive(Debug, Clone, PartialEq)]
pub enum Call {
    /// 换了应用（选着的没变不记）。
    Select(Option<String>),
    Capabilities(Capabilities),
    Focus(bool),
    Deactivate,
    Reset,
    Key {
        code: u32,
        release: bool,
        context: bool,
    },
    Poll,
    Acknowledge(u64),
}

/// 一次按下的脚本化答复。
pub struct Answer {
    pub outcome: KeyOutcome,

    pub commit: Option<String>,

    pub frame: Frame,

    pub english: Option<bool>,
}

/// 照脚本回话的 Server：按下按 `presses` 依次答；抬起与轮询照真 Server 的样子回当前帧。
#[derive(Default)]
pub struct FakeServer {
    pub calls: Vec<Call>,

    pub presses: VecDeque<Answer>,

    /// 抬起的特别答复（单击 Shift 切换）；没有就回放行加当前帧。
    pub releases: VecDeque<Answer>,

    /// 下一次轮询要给的新帧（重排结果到了）。
    pub polled: Option<Frame>,

    /// Server 挂了：所有请求都失败。
    pub down: bool,

    /// 选着的应用。
    app: Option<String>,

    frame: Frame,

    revision: u64,

    connected: bool,

    retry_at: Option<Instant>,
}

impl FakeServer {
    fn reply(
        &mut self,
        outcome: KeyOutcome,
        commit: Option<String>,
        english: Option<bool>,
    ) -> Reply {
        self.revision += 1;
        Reply {
            outcome,
            commit,
            frame: self.frame.clone(),
            identity: Some(identity(self.revision)),
            english,
            translation_pending: false,
            ..Reply::default()
        }
    }

    fn check(&mut self) -> Result<(), ClientError> {
        if self.down {
            self.connected = false;
            self.retry_at = Some(Instant::now() + Duration::from_secs(2));
            return Err(ClientError::Closed);
        }
        self.connected = true;
        Ok(())
    }
}

impl Server for FakeServer {
    fn select(&mut self, app: Option<&str>) {
        if self.app.as_deref() != app {
            self.app = app.map(str::to_owned);
            self.calls.push(Call::Select(self.app.clone()));
        }
    }

    fn app(&self) -> Option<&str> {
        self.app.as_deref()
    }

    fn capabilities(&mut self, capabilities: Capabilities) -> Result<(), ClientError> {
        self.calls.push(Call::Capabilities(capabilities));
        Ok(())
    }

    fn focus(&mut self, focused: bool) -> Result<Reply, ClientError> {
        self.calls.push(Call::Focus(focused));
        self.check()?;
        if !focused {
            self.frame = Frame::default();
        }
        Ok(self.reply(KeyOutcome::Passthrough, None, None))
    }

    fn deactivate(&mut self) -> Result<Reply, ClientError> {
        self.calls.push(Call::Deactivate);
        self.check()?;
        self.frame = Frame::default();
        Ok(self.reply(KeyOutcome::Passthrough, None, None))
    }

    fn reset(&mut self) -> Result<Reply, ClientError> {
        self.calls.push(Call::Reset);
        self.check()?;
        self.frame = Frame::default();
        Ok(self.reply(KeyOutcome::Passthrough, None, None))
    }

    fn key(
        &mut self,
        event: KeyEvent,
        release: bool,
        surrounding: Option<AppSurrounding>,
    ) -> Result<Reply, ClientError> {
        self.calls.push(Call::Key {
            code: event.virtual_key,
            release,
            context: surrounding.is_some(),
        });
        self.check()?;
        let script = if release {
            &mut self.releases
        } else {
            &mut self.presses
        };
        match script.pop_front() {
            Some(answer) => {
                self.frame = answer.frame;
                Ok(self.reply(answer.outcome, answer.commit, answer.english))
            }
            None => Ok(self.reply(KeyOutcome::Passthrough, None, None)),
        }
    }

    fn poll(&mut self) -> Result<Reply, ClientError> {
        self.calls.push(Call::Poll);
        self.check()?;
        let mut reply = match self.polled.take() {
            Some(frame) => {
                self.frame = frame;
                self.reply(KeyOutcome::Passthrough, None, None)
            }
            // 帧没变：版本号沿用
            None => {
                let mut reply = self.reply(KeyOutcome::Passthrough, None, None);
                self.revision -= 1;
                reply.identity = Some(identity(self.revision));
                reply
            }
        };
        reply.outcome = KeyOutcome::Passthrough;
        Ok(reply)
    }

    fn acknowledge(
        &mut self,
        identity: DisplayIdentity,
        _senses: Vec<(usize, usize)>,
    ) -> Result<(), ClientError> {
        self.calls.push(Call::Acknowledge(identity.revision));
        Ok(())
    }

    fn connected(&self) -> bool {
        self.connected
    }

    fn retry_at(&self) -> Option<Instant> {
        self.retry_at
    }
}

/// 假合成器记下的动作。
#[derive(Debug, Clone, PartialEq)]
pub enum Out {
    Grab(bool),
    Keymap,
    Forward(u32, bool),
    Modifiers(u32),
    Text(String, String),
    Show(Vec<String>),
    Hide,
}

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
