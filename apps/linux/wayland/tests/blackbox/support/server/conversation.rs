//! 一条连接上的对话：照真 Server（`apps/linux/server/src/ipc/connection.rs`）的壳回包，按下与测试允许的鼠标操作照脚本答。
//!
//! 没想到的消息不回个空包糊弄过去：真 Server 会断开连接的（握手不合规矩、报能力之前发事件、轮询没开的会话），
//! 与这些测试里前端不该发的（未允许的鼠标操作、托盘、没要过的选区回话、认不出的消息），都当违规交给调用方，
//! 调用方记下来、断开连接，测试结束时报出来。

use std::collections::HashMap;

use qingjian_platform::protocol::linux::{
    DisplayAcknowledged, DisplayIdentity, LINUX_UI_PROTOCOL, LinuxEvent,
};
use qingjian_platform::protocol::{Frame, KeyOutcome, PROTOCOL_VERSION, ServerMessage, SessionId};
use serde_json::{Value, json};

use super::answer::translation;
use super::opened::Opened;
use super::{Answer, Script};

/// 选区最多多少个字，与真 Server 的 `receive_selection` 一样：再长放行。
const MAX_SELECTION_CHARS: usize = 500;

/// 一次展示回执最多几对义项，再多真 Server 断开连接。
const MAX_SENSES: usize = 128;

/// 违规：一句说明。
type Violation = String;

/// 右键菜单回的几项：编号与字，与真 Server 的候选菜单一样带编号。
pub const MENU: [(u32, &str); 2] = [(1, "置顶"), (5, "删除")];

#[derive(Default)]
pub struct Conversation {
    /// `LinuxHello` 报的连接代次，回包的展示身份里原样带回。
    generation: u64,

    sessions: HashMap<u64, Opened>,

    /// 现在的帧：抬键、轮询、焦点这些不改帧的回包照真 Server 的样子带着它。
    frame: Frame,

    /// 帧的版本号，每个事件的回包加一；轮询不加（帧没变，前端据此不重画），交出译文那一次加一。
    revision: u64,

    /// 选区翻译在等译文：回包上的 `translation_pending`。
    pending: bool,

    /// 发出去、前端还没回话的选区请求。
    requested: Option<u64>,
}

impl Conversation {
    /// 回一条消息：`Ok(None)` 是没有回包的那几种（展示回执、关会话）。
    pub fn reply(
        &mut self,
        message: &Value,
        script: &mut Script,
    ) -> Result<Option<Value>, Violation> {
        let (name, body) = message
            .as_object()
            .filter(|object| object.len() == 1)
            .and_then(|object| object.iter().next())
            .ok_or_else(|| format!("认不出的消息：{message}"))?;
        match name.as_str() {
            "OpenSession" => self.open(body).map(Some),
            "LinuxHello" => self.hello(body).map(Some),
            "LinuxEvent" => self.event(body, script).map(Some),
            "Poll" => self.poll(body, script).map(Some),
            "DisplayAcknowledged" => self.acknowledge(body).map(|()| None),
            "CloseSession" => {
                let session = number(body, "session")?;
                self.sessions
                    .remove(&session)
                    .ok_or_else(|| format!("关了没开的会话 {session}"))?;
                Ok(None)
            }
            _ => Err(format!("没想到的消息：{message}")),
        }
    }

    fn open(&mut self, body: &Value) -> Result<Value, Violation> {
        let session = number(body, "session")?;
        if body["protocol"] != PROTOCOL_VERSION {
            return Err(format!("OpenSession 报的协议版本不对：{body}"));
        }
        let fresh = Opened {
            context: None,
            capabilities: None,
        };
        if self.sessions.insert(session, fresh).is_some() {
            return Err(format!("会话 {session} 开了两次"));
        }
        Ok(json!({"Update": {"session": session, "frame": Frame::default(), "linux_ui": {}}}))
    }

    /// 握手：真 Server 要求上下文非空、不超过 64 字节、同一条连接上不重复，代次大于 0 且整条连接一致。
    fn hello(&mut self, body: &Value) -> Result<Value, Violation> {
        let session = number(body, "session")?;
        let generation = number(body, "generation")?;
        let context = body["context"].as_str().unwrap_or_default().to_owned();
        if body["version"] != LINUX_UI_PROTOCOL {
            return Err(format!(
                "LinuxHello 报的版本不是 {LINUX_UI_PROTOCOL}：{body}"
            ));
        }
        if generation == 0 || (self.generation != 0 && generation != self.generation) {
            return Err(format!("LinuxHello 的连接代次不对：{body}"));
        }
        let taken = self
            .sessions
            .values()
            .any(|opened| opened.context.as_deref() == Some(context.as_str()));
        if context.is_empty() || context.len() > 64 || taken {
            return Err(format!("LinuxHello 的上下文不合规矩：{body}"));
        }
        let opened = self
            .sessions
            .get_mut(&session)
            .filter(|opened| opened.context.is_none())
            .ok_or_else(|| format!("会话 {session} 没开或握过手了"))?;
        opened.context = Some(context);
        self.generation = generation;
        Ok(json!({"LinuxHello": {"session": session, "page_size": 9}}))
    }

    fn event(&mut self, body: &Value, script: &mut Script) -> Result<Value, Violation> {
        let session = number(body, "session")?;
        let event: LinuxEvent = serde_json::from_value(body["event"].clone())
            .map_err(|error| format!("解不开的 LinuxEvent（{error}）：{body}"))?;
        let opened = self.opened(session)?;
        match (&event, opened.capabilities) {
            (LinuxEvent::Capabilities(capabilities), reported) => {
                opened.capabilities = Some(*capabilities);
                // 能力变了真 Server 丢掉组句与翻译
                if reported.is_some_and(|reported| reported != *capabilities) {
                    self.clear();
                }
                return Ok(self.result(session, KeyOutcome::Passthrough, None));
            }
            (_, None) => return Err(format!("报能力之前发了事件：{body}")),
            _ => {}
        }
        if let Some(reply) = self.panel(&event, session, script)? {
            return Ok(reply);
        }
        let selection_supported = matches!(
            event,
            LinuxEvent::Key {
                release: false,
                selection_supported: true,
                ..
            }
        );
        let (outcome, commit) = match event {
            LinuxEvent::Key { release: false, .. }
            | LinuxEvent::Candidate { .. }
            | LinuxEvent::Page { .. }
            | LinuxEvent::MenuPick { .. } => {
                let answer = script
                    .presses
                    .pop_front()
                    .unwrap_or_else(Answer::passthrough);
                if let Some(request) = answer.select {
                    if !selection_supported {
                        return Err(format!("脚本要选区，这一键却没报能读选区：{body}"));
                    }
                    self.requested = Some(request);
                    return Ok(
                        json!({"RequestSelection": {"session": session, "request": request}}),
                    );
                }
                if let Some(frame) = answer.frame {
                    self.frame = frame;
                    self.pending = false;
                }
                (answer.outcome, answer.commit)
            }
            LinuxEvent::Key { release: true, .. } | LinuxEvent::Focus { focused: true } => {
                (KeyOutcome::Passthrough, None)
            }
            LinuxEvent::Selection { request, text } => {
                if self.requested.take() != Some(request) {
                    return Err(format!("没要过的选区回话：{body}"));
                }
                if text.trim().is_empty() || text.chars().count() > MAX_SELECTION_CHARS {
                    (KeyOutcome::Passthrough, None)
                } else {
                    self.frame = translation("翻译中…");
                    self.pending = true;
                    (KeyOutcome::Consumed, None)
                }
            }
            LinuxEvent::Focus { focused: false }
            | LinuxEvent::Deactivate { .. }
            | LinuxEvent::Reset => {
                self.clear();
                (KeyOutcome::Passthrough, None)
            }
            other => return Err(format!("这些测试里前端不该发 {other:?}")),
        };
        Ok(self.result(session, outcome, commit))
    }

    /// 轮询：帧没变时版本号不动；选区翻译在等、测试交出了译文，就换上译文那一帧。
    fn poll(&mut self, body: &Value, script: &mut Script) -> Result<Value, Violation> {
        let session = number(body, "session")?;
        self.opened(session)?;
        if self.pending
            && let Some(text) = script.translation.take()
        {
            self.frame = translation(&text);
            self.pending = false;
            self.revision += 1;
        }
        let mut reply = serde_json::to_value(ServerMessage::Update {
            session: SessionId(session),
            frame: self.frame.clone(),
            commit: None,
        })
        .expect("回包能转成 JSON");
        self.decorate(&mut reply["Update"], session);
        Ok(reply)
    }

    /// 展示回执：身份要是这条连接、这个会话的，一次不超过 128 对。版本号不核对：前端回的是它画的那一帧，帧可能已经又换了。
    fn acknowledge(&mut self, body: &Value) -> Result<(), Violation> {
        let ack: DisplayAcknowledged = serde_json::from_value(body.clone())
            .map_err(|error| format!("解不开的展示回执（{error}）：{body}"))?;
        let generation = self.generation;
        let opened = self.opened(ack.session.0)?;
        if ack.identity.generation != generation
            || opened.context.as_deref() != Some(ack.identity.context.as_str())
            || ack.senses.len() > MAX_SENSES
        {
            return Err(format!("展示回执对不上：{body}"));
        }
        Ok(())
    }

    /// 握过手的会话。
    fn opened(&mut self, session: u64) -> Result<&mut Opened, Violation> {
        self.sessions
            .get_mut(&session)
            .filter(|opened| opened.context.is_some())
            .ok_or_else(|| format!("会话 {session} 没开或还没握手"))
    }

    /// 组句与翻译都丢掉。
    fn clear(&mut self) {
        self.frame = Frame::default();
        self.pending = false;
        self.requested = None;
    }

    /// 一个事件的回包：`KeyResult` 带着现在的帧，版本号加一。
    fn result(&mut self, session: u64, outcome: KeyOutcome, commit: Option<String>) -> Value {
        self.revision += 1;
        let mut reply = serde_json::to_value(ServerMessage::KeyResult {
            session: SessionId(session),
            outcome,
            commit,
            frame: self.frame.clone(),
            english: None,
        })
        .expect("回包能转成 JSON");
        self.decorate(&mut reply["KeyResult"], session);
        reply
    }

    /// Linux Server 另附在回包上的：展示身份（前端靠它判断帧换没换、回执原样带回）与帧上的 `translation_pending`。
    fn decorate(&self, body: &mut Value, session: u64) {
        let context = self
            .sessions
            .get(&session)
            .and_then(|opened| opened.context.clone())
            .unwrap_or_default();
        body["identity"] = json!({
            "generation": self.generation,
            "context": context,
            "revision": self.revision,
        });
        body["frame"]["translation_pending"] = json!(self.pending);
    }

    /// 候选窗上的鼠标操作（点候选、翻页、问菜单、点菜单项）像真 Server 那样先核对展示身份：对不上当前帧回 `Ignored`；
    /// 须先由测试逐种允许，问菜单只回那几项、不推进身份。别的事件是 `None`，照常处理。
    fn panel(
        &self,
        event: &LinuxEvent,
        session: u64,
        script: &Script,
    ) -> Result<Option<Value>, Violation> {
        let (name, identity) = match event {
            LinuxEvent::Candidate { identity, .. } => ("Candidate", identity),
            LinuxEvent::Page { identity, .. } => ("Page", identity),
            LinuxEvent::Menu { identity, .. } => ("Menu", identity),
            LinuxEvent::MenuPick { identity, .. } => ("MenuPick", identity),
            _ => return Ok(None),
        };
        if !script.pointer_events.contains(name) {
            return Err(format!("这个测试没允许 {name}：{event:?}"));
        }
        let current = DisplayIdentity {
            generation: self.generation,
            context: self.sessions[&session]
                .context
                .clone()
                .expect("事件已核对握手"),
            revision: self.revision,
        };
        if *identity != current {
            return Ok(Some(json!({"Ignored": {"session": session}})));
        }
        if !matches!(event, LinuxEvent::Menu { .. }) {
            return Ok(None);
        }
        let items: Vec<Value> = MENU
            .iter()
            .map(|(id, label)| json!({"id": id, "label": label}))
            .collect();
        Ok(Some(json!({"Menu": {"session": session, "items": items}})))
    }
}

fn number(body: &Value, key: &str) -> Result<u64, Violation> {
    body[key]
        .as_u64()
        .ok_or_else(|| format!("缺 {key} 或不是数：{body}"))
}
