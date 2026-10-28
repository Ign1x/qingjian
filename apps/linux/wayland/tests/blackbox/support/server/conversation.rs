//! 一条连接上的对话：照真 Server（`apps/linux/server/src/ipc/connection.rs`）的壳回包，按下与候选窗上的点击照脚本答。

use std::collections::VecDeque;
use std::sync::{Mutex, PoisonError};

use qingjian_platform::protocol::linux::{DisplayIdentity, LinuxEvent};
use qingjian_platform::protocol::{Frame, KeyOutcome, ServerMessage, SessionId};
use serde_json::{Value, json};

use super::Answer;

/// 右键菜单回的几项：编号与字，与真 Server 的候选菜单一样带编号。
pub const MENU: [(u32, &str); 2] = [(1, "置顶"), (5, "删除")];

#[derive(Default)]
pub struct Conversation {
    /// `LinuxHello` 报的连接代次，回包的展示身份里原样带回。
    generation: u64,

    context: String,

    /// 现在的帧：抬键、轮询、焦点这些不改帧的回包照真 Server 的样子带着它。
    frame: Frame,

    /// 帧的版本号，每个回包加一；轮询不加（帧没变，前端据此不重画）。
    revision: u64,
}

impl Conversation {
    /// 回一条消息；`DisplayAcknowledged` 与 `CloseSession` 没有回包。
    pub fn reply(&mut self, message: &Value, presses: &Mutex<VecDeque<Answer>>) -> Option<Value> {
        if let Some(open) = message.get("OpenSession") {
            return Some(json!({"Update": {
                "session": open["session"],
                "frame": Frame::default(),
                "linux_ui": {},
            }}));
        }
        if let Some(hello) = message.get("LinuxHello") {
            self.generation = hello["generation"].as_u64().unwrap_or_default();
            hello["context"]
                .as_str()
                .unwrap_or_default()
                .clone_into(&mut self.context);
            return Some(json!({"LinuxHello": {"session": hello["session"], "page_size": 9}}));
        }
        if let Some(request) = message.get("LinuxEvent") {
            let event: LinuxEvent =
                serde_json::from_value(request["event"].clone()).expect("合法的 LinuxEvent");
            if let Some(reply) = self.panel(&event, request) {
                return Some(reply);
            }
            let (outcome, commit) = self.handle(event, presses);
            self.revision += 1;
            let mut reply = serde_json::to_value(ServerMessage::KeyResult {
                session: session(request),
                outcome,
                commit,
                frame: self.frame.clone(),
                english: None,
            })
            .expect("回包能转成 JSON");
            reply["KeyResult"]["identity"] = self.identity();
            return Some(reply);
        }
        if let Some(poll) = message.get("Poll") {
            let mut reply = serde_json::to_value(ServerMessage::Update {
                session: session(poll),
                frame: self.frame.clone(),
                commit: None,
            })
            .expect("回包能转成 JSON");
            reply["Update"]["identity"] = self.identity();
            return Some(reply);
        }
        None
    }

    /// 一个事件的处置：按下与候选窗上的点击、翻页、菜单项照脚本（没有脚本就放行），失焦、失活、重置清掉组句，
    /// 别的放行、帧不变。
    fn handle(
        &mut self,
        event: LinuxEvent,
        presses: &Mutex<VecDeque<Answer>>,
    ) -> (KeyOutcome, Option<String>) {
        match event {
            LinuxEvent::Key { release: false, .. }
            | LinuxEvent::Candidate { .. }
            | LinuxEvent::Page { .. }
            | LinuxEvent::MenuPick { .. } => {
                let answer = presses
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .pop_front()
                    .unwrap_or_else(Answer::passthrough);
                if let Some(frame) = answer.frame {
                    self.frame = frame;
                }
                (answer.outcome, answer.commit)
            }
            LinuxEvent::Focus { focused: false }
            | LinuxEvent::Deactivate { .. }
            | LinuxEvent::Reset => {
                self.frame = Frame::default();
                (KeyOutcome::Passthrough, None)
            }
            _ => (KeyOutcome::Passthrough, None),
        }
    }

    /// 展示身份：Linux Server 另附在回包上的，前端靠它判断帧换没换、回执原样带回。
    fn identity(&self) -> Value {
        json!({
            "generation": self.generation,
            "context": self.context,
            "revision": self.revision,
        })
    }

    /// 候选窗上的鼠标操作（点候选、翻页、问菜单、点菜单项）像真 Server 那样先核对展示身份：对不上当前帧回 `Ignored`；
    /// 问菜单只回那几项，不推进身份。别的事件是 `None`，照常处理。
    fn panel(&self, event: &LinuxEvent, request: &Value) -> Option<Value> {
        let identity = match event {
            LinuxEvent::Candidate { identity, .. }
            | LinuxEvent::Page { identity, .. }
            | LinuxEvent::Menu { identity, .. }
            | LinuxEvent::MenuPick { identity, .. } => identity,
            _ => return None,
        };
        let session = &request["session"];
        let current = DisplayIdentity {
            generation: self.generation,
            context: self.context.clone(),
            revision: self.revision,
        };
        if *identity != current {
            return Some(json!({"Ignored": {"session": session}}));
        }
        if !matches!(event, LinuxEvent::Menu { .. }) {
            return None;
        }
        let items: Vec<Value> = MENU
            .iter()
            .map(|(id, label)| json!({"id": id, "label": label}))
            .collect();
        Some(json!({"Menu": {"session": session, "items": items}}))
    }
}

fn session(body: &Value) -> SessionId {
    SessionId(body["session"].as_u64().unwrap_or_default())
}
