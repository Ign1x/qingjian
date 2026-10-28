//! Linux 版本化显示初始化与实际呈现确认，Windows 协议保持不变。
mod annotation;
#[cfg(test)]
mod tests;

use self::annotation::annotate_frame;
use super::{Answer, Router};
use qingjian_core::Translation;
use qingjian_platform::protocol::linux::{
    DisplayAcknowledged, DisplayIdentity, Features, LINUX_UI_PROTOCOL_MIN, LinuxEvent, LinuxRequest,
};
use qingjian_platform::protocol::{ClientMessage, ServerMessage, SessionId};
use serde_json::{Value, json};

impl Router {
    /// `OpenSession` 回包的 `linux_ui`，`LinuxHello` 的回包在它上面换上会话编号与握手的版本。
    /// 这里的版本固定报还认的最老一版：握手之前不知道前端是哪一版，Fcitx5 / Fcitx4 插件要它等于 3。
    pub fn display_settings(&self) -> Value {
        json!({"version": LINUX_UI_PROTOCOL_MIN, "preedit": self.config.preedit})
    }

    pub fn handle_linux(&mut self, value: Value) -> Option<Value> {
        if let Some(body) = value.get("DisplayReporting") {
            let session: SessionId = serde_json::from_value(body.get("session")?.clone()).ok()?;
            let identity: DisplayIdentity =
                serde_json::from_value(body.get("identity")?.clone()).ok()?;
            let features: Features = body
                .get("features")
                .and_then(|value| serde_json::from_value(value.clone()).ok())
                .unwrap_or_default();
            let info = self.sessions.get_mut(&session)?;
            info.display_identity = Some(identity);
            info.features = features;
            info.display_frame = None;
            info.last_frame = None;
            if self.focused == Some(session) {
                self.engine.note_displayed(std::iter::empty());
            }
            return None;
        }
        if let Some(body) = value.get("DisplayAcknowledged") {
            let ack: DisplayAcknowledged = serde_json::from_value(body.clone()).ok()?;
            self.acknowledge_display(ack);
            return None;
        }
        let response = if let Some(body) = value.get("LinuxEvent") {
            let request: LinuxRequest = serde_json::from_value(body.clone()).ok()?;
            if let LinuxEvent::Candidate { identity, .. }
            | LinuxEvent::Page { identity, .. }
            | LinuxEvent::Menu { identity, .. }
            | LinuxEvent::MenuPick { identity, .. } = &request.event
                && !self.valid_panel_event(request.session, identity)
            {
                // 旧鼠标事件不能改变其他会话的焦点、显示身份或学习回报。
                return Some(json!({"Ignored": {"session": request.session}}));
            }
            match self.linux_event(request)? {
                Answer::Message(message) => message,
                Answer::Menu(menu) => return Some(json!({ "Menu": menu })),
            }
        } else {
            let message: ClientMessage = serde_json::from_value(value).ok()?;
            self.handle(message)?
        };
        let mut value = serde_json::to_value(&response).ok()?;
        let pending = self
            .translation
            .as_ref()
            .is_some_and(|job| job.result.is_none());
        // 只给握手报了画得了键位提示的会话；没报的（Fcitx5 / Fcitx4 插件、IBus 前端）回包上什么都不附
        let key_hint = match &response {
            ServerMessage::KeyResult { session, .. } | ServerMessage::Update { session, .. } => {
                self.key_hint_for(*session)
            }
            _ => None,
        };
        if let ServerMessage::KeyResult { session, frame, .. }
        | ServerMessage::Update { session, frame, .. } = &response
        {
            let body = value.as_object_mut()?.values_mut().next()?;
            body["frame"]["translation_pending"] = json!(pending);
            annotate_frame(frame, &mut body["frame"]);
            if let Some(mode) = self.input_mode(*session) {
                body["mode"] = json!(mode);
            }
            if let Some(key_hint) = &key_hint {
                body["key_hint"] = json!(key_hint);
            }
        }
        match &response {
            ServerMessage::KeyResult { session, frame, .. }
            | ServerMessage::Update { session, frame, .. } => {
                let polled = matches!(response, ServerMessage::Update { .. });
                let info = self.sessions.get_mut(session)?;
                if let Some(identity) = &mut info.display_identity {
                    // 插件组句期间定时 Poll：帧与键位提示都没变就不是新的展示，身份沿用，已回报的曝光照算
                    if polled
                        && info.last_frame.as_ref() == Some(frame)
                        && info.last_key_hint == key_hint
                    {
                        value.as_object_mut()?.values_mut().next()?["identity"] = json!(identity);
                        return Some(value);
                    }
                    self.display_revision += 1;
                    if self.focused == Some(*session) {
                        self.engine.note_displayed(std::iter::empty());
                    }
                    identity.revision = self.display_revision;
                    info.display_frame = (!info.private && info.active).then(|| frame.clone());
                    info.last_frame = Some(frame.clone());
                    info.last_key_hint = key_hint;
                    value.as_object_mut()?.values_mut().next()?["identity"] = json!(identity);
                }
            }
            ServerMessage::Committed { session, .. } => {
                self.display_revision += 1;
                if let Some(info) = self.sessions.get_mut(session) {
                    info.display_frame = None;
                    info.last_frame = None;
                    if let Some(identity) = &mut info.display_identity {
                        identity.revision = self.display_revision;
                    }
                }
            }
            _ => {}
        }
        Some(value)
    }

    fn acknowledge_display(&mut self, ack: DisplayAcknowledged) {
        if self.focused != Some(ack.session) || ack.senses.len() > 128 {
            return;
        }
        let Some(info) = self.sessions.get(&ack.session) else {
            return;
        };
        if info.private || !info.active || info.display_identity.as_ref() != Some(&ack.identity) {
            return;
        }
        let Some(frame) = &info.display_frame else {
            return;
        };
        // 所有索引先校验，坏回报不能产生半份记录。
        if ack.senses.iter().any(|(row, sense)| {
            frame
                .candidates
                .items
                .get(*row)
                .filter(|c| !c.text.is_empty())
                .and_then(|c| c.translation.as_ref())
                .is_none_or(|t| *sense >= t.senses().len())
        }) {
            return;
        }
        let candidates: Vec<_> = frame
            .candidates
            .items
            .iter()
            .enumerate()
            .filter_map(|(row, candidate)| {
                let translation = candidate.translation.as_ref()?;
                let senses = translation
                    .senses()
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| ack.senses.contains(&(row, *i)))
                    .map(|(_, s)| s.clone())
                    .collect::<Vec<_>>();
                if senses.is_empty() {
                    return None;
                }
                let mut candidate = candidate.clone();
                candidate.translation = Some(Translation::new(translation.language, senses));
                Some(candidate)
            })
            .collect();
        self.engine.note_displayed(candidates.iter());
    }
}
