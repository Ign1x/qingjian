//! Linux 事件决策；Fcitx 插件只报告事实并应用返回值。
use super::{
    Answer, Router,
    key::{Effect, codes},
};
use qingjian_core::SurroundingText;
use qingjian_core::candidate::arrows::Focus;
use qingjian_platform::protocol::linux::{
    DisplayIdentity, LinuxEvent, LinuxRequest, MenuReply, ModeCommand,
};
use qingjian_platform::protocol::{KeyOutcome, ServerMessage, SessionId};

impl Router {
    pub(super) fn linux_event(&mut self, request: LinuxRequest) -> Option<Answer> {
        let session = request.session;
        if !self.sessions.contains_key(&session) {
            return None;
        }
        let mut outcome = KeyOutcome::Passthrough;
        let mut commit = None;
        let mut switched = None;
        match request.event {
            LinuxEvent::Reset => {
                if self.focused == Some(session) {
                    self.end_translation();
                }
                self.discard_session(session);
            }
            LinuxEvent::Capabilities(caps) => {
                let disabled = caps.password || caps.disabled;
                let private = disabled || caps.sensitive;
                if self.sessions[&session].capabilities != Some(caps) {
                    // 即使 Sensitive → Password 仍为 private，也必须丢弃全部旧输入。
                    self.discard_session(session);
                    self.set_privacy(session, private);
                    let info = self.sessions.get_mut(&session)?;
                    info.capabilities = Some(caps);
                    info.disabled = disabled;
                }
            }
            LinuxEvent::Focus { focused } => {
                let info = self.sessions.get_mut(&session)?;
                let was_active = info.active;
                info.active = focused;
                info.switch_tap = None;
                info.display_frame = None;
                info.last_frame = None;
                if !focused && self.focused == Some(session) {
                    self.end_translation();
                    self.engine.note_displayed(std::iter::empty());
                    self.engine.cancel_prediction();
                }
                if focused {
                    let was_focused = self.focused == Some(session);
                    self.ensure_focus(session);
                    if was_focused && !was_active {
                        self.request_current_prediction();
                    }
                }
            }
            LinuxEvent::Deactivate {
                focus_out,
                client_preedit,
                capability_changed,
            } => {
                if self.focused == Some(session) {
                    self.end_translation();
                }
                if capability_changed || self.sessions[&session].disabled {
                    self.discard_session(session);
                } else {
                    self.ensure_focus(session);
                    let text =
                        (!self.engine.composition().is_empty()).then(|| self.engine.take_raw());
                    if !(focus_out && client_preedit) {
                        commit = text;
                    }
                    self.reset_composition();
                }
                let info = self.sessions.get_mut(&session)?;
                info.active = false;
                info.switch_tap = None;
                info.display_frame = None;
                info.last_frame = None;
                self.flush_learning();
            }
            LinuxEvent::Key {
                mut event,
                release,
                surrounding,
                selection_supported,
            } => {
                if self.sessions[&session].disabled
                    || self.sessions[&session].capabilities.is_none()
                {
                    return Some(
                        ServerMessage::KeyResult {
                            session,
                            outcome,
                            commit,
                            frame: Default::default(),
                            english: None,
                        }
                        .into(),
                    );
                }
                self.ensure_focus(session);
                self.notice = None;
                if !release && self.translation.is_some() {
                    let (outcome, commit) = self.review_translation(&event);
                    return Some(
                        ServerMessage::KeyResult {
                            session,
                            outcome,
                            commit,
                            frame: self.current_frame(),
                            english: None,
                        }
                        .into(),
                    );
                }
                if !release
                    && selection_supported
                    && self.engine.composition().is_empty()
                    && self.engine.prediction_enabled()
                    && !self.sessions[&session].private
                    && self.sessions[&session].active
                    && self.matches_translate_combo(&event)
                {
                    return Some(self.request_selection(session).into());
                }
                if release && self.translation.is_some() {
                    return Some(
                        ServerMessage::KeyResult {
                            session,
                            outcome,
                            commit,
                            frame: self.current_frame(),
                            english: None,
                        }
                        .into(),
                    );
                }
                let surrounding: Option<SurroundingText> = (!self.sessions[&session].private)
                    .then_some(surrounding)
                    .flatten()
                    .map(Into::into);
                let keys = self.config.switch_keys();
                let info = self.sessions.get_mut(&session)?;
                if release {
                    if info.switch_tap == Some(event.virtual_key) {
                        info.switch_tap = None;
                        commit = self.toggle_english(session);
                        switched = Some(self.sessions[&session].english);
                        outcome = KeyOutcome::Consumed;
                    }
                } else if keys.ctrl_alt_space && codes::is_ctrl_alt_space(&event) {
                    info.switch_tap = None;
                    commit = self.toggle_english(session);
                    switched = Some(self.sessions[&session].english);
                    outcome = KeyOutcome::Consumed;
                } else {
                    info.switch_tap = codes::switch_tap(keys, &event);
                    event.modifiers.english_mode = info.english;
                    // 应用里光标前的文字：还没上屏过时自动加空格拿它判上文，本地整句模型拿它当前文（同 Windows、macOS 壳）；
                    // 应用不报（或私密输入框）时为空，按本会话上屏的字算。Core 在组句清空时丢掉它，所以每键都给
                    self.engine.set_rescoring_context(
                        surrounding
                            .as_ref()
                            .map(|text| text.before.clone())
                            .filter(|before| !before.is_empty()),
                    );
                    if !codes::is_tap_modifier(event.virtual_key)
                        && !self.engine.composition().is_empty()
                        && self.config.cloud_enabled
                        && self.engine.prediction_enabled()
                        && self.matches_predict_combo(&event)
                    {
                        // 组合键一按就吃掉，联想没发出去也不还给应用（与 Windows 壳一致）；
                        // 显式要求刷新时回到第一页，让新的 AI 首选可以置顶。
                        if self.request_manual_prediction(surrounding) {
                            self.highlight = 0;
                            self.navigated = false;
                        }
                        outcome = KeyOutcome::Consumed;
                    } else {
                        let effect = if codes::is_tap_modifier(event.virtual_key) {
                            Effect::Passthrough
                        } else {
                            self.apply_key(&event)
                        };
                        match effect {
                            Effect::Changed(text) => {
                                self.recompose(surrounding);
                                commit = text;
                                outcome = KeyOutcome::Consumed;
                            }
                            Effect::Navigated => outcome = KeyOutcome::Consumed,
                            Effect::Passthrough => {}
                        }
                    }
                }
            }
            LinuxEvent::Candidate { identity, index } => {
                // 格号按这一帧算：横排矩阵展开着时是视口里的第几格
                let cells = self.frame_cells();
                if self.valid_panel_event(session, &identity) && index < cells.len() {
                    if self.translation.is_some() {
                        if index == 0 {
                            commit = self.translation.as_ref().and_then(|job| job.result.clone());
                            if commit.is_some() {
                                self.end_translation();
                            }
                        }
                        outcome = KeyOutcome::Consumed;
                    } else {
                        commit = self.commit_index(cells.start + index);
                        if commit.is_some() {
                            self.recompose(None);
                            outcome = KeyOutcome::Consumed;
                        }
                    }
                }
            }
            LinuxEvent::Selection { request, text } => {
                outcome = self.receive_selection(session, request, text);
            }
            LinuxEvent::Page { identity, next } => {
                if self.valid_panel_event(session, &identity) {
                    self.page(if next { 1 } else { -1 });
                    outcome = KeyOutcome::Consumed;
                }
            }
            LinuxEvent::Menu { identity, index } => {
                // 只是问一句：不动组句，也不推进展示身份，之后的 MenuPick 才对得上这一帧
                let items = if self.valid_panel_event(session, &identity) {
                    self.menu_items(index)
                } else {
                    Vec::new()
                };
                return Some(Answer::Menu(MenuReply { session, items }));
            }
            LinuxEvent::MenuPick {
                identity,
                index,
                action,
            } => {
                if self.valid_panel_event(session, &identity) && self.pick_menu_item(index, action)
                {
                    outcome = KeyOutcome::Consumed;
                }
            }
            LinuxEvent::Indicator(command) => {
                // 托盘点的是这个会话：中 / 英要交出的是它的组句，先装进 Engine
                self.ensure_focus(session);
                match command {
                    ModeCommand::ToggleEnglish => {
                        if self.config.english_mode {
                            self.sessions.get_mut(&session)?.switch_tap = None;
                            commit = self.toggle_english(session);
                            switched = Some(self.sessions[&session].english);
                            outcome = KeyOutcome::Consumed;
                        }
                    }
                    ModeCommand::TogglePunctuation => {
                        self.toggle_punctuation(session);
                        outcome = KeyOutcome::Consumed;
                    }
                    ModeCommand::ToggleTraditional => {
                        self.toggle_traditional();
                        outcome = KeyOutcome::Consumed;
                    }
                }
            }
        }
        let frame = if self.focused == Some(session) && !self.sessions[&session].disabled {
            self.current_frame()
        } else {
            Default::default()
        };
        // 选词没选完一段拼音时 Core 把词留在组句里、上屏文本是空串
        let commit = commit.filter(|text| !text.is_empty());
        Some(
            ServerMessage::KeyResult {
                session,
                outcome,
                commit,
                frame,
                english: switched,
            }
            .into(),
        )
    }

    pub(super) fn valid_panel_event(&self, session: SessionId, identity: &DisplayIdentity) -> bool {
        let Some(info) = self.sessions.get(&session) else {
            return false;
        };
        self.focused == Some(session)
            && info.active
            && !info.disabled
            && info.display_identity.as_ref() == Some(identity)
    }

    /// 切换中英：组句中的拼音原样上屏。
    fn toggle_english(&mut self, session: SessionId) -> Option<String> {
        let info = self.sessions.get_mut(&session).expect("known session");
        info.english = !info.english;
        let commit = (!self.engine.composition().is_empty()).then(|| self.engine.take_raw());
        self.reset_composition();
        commit
    }

    fn discard_session(&mut self, session: SessionId) {
        let info = self.sessions.get_mut(&session).expect("known session");
        info.switch_tap = None;
        info.display_frame = None;
        info.last_frame = None;
        info.composed = None;
        info.highlight = 0;
        info.navigated = false;
        info.focus = Focus::default();
        info.grid = None;
        if let Some(identity) = &mut info.display_identity {
            self.display_revision += 1;
            identity.revision = self.display_revision;
        }
        if self.focused == Some(session) {
            self.stop_rescoring();
            self.engine.discard_input();
            self.composed = None;
            self.sentence = None;
            self.notice = None;
            self.highlight = 0;
            self.navigated = false;
            self.focus = Focus::default();
            self.grid = None;
        } else {
            info.engine.discard_input();
        }
    }
}
