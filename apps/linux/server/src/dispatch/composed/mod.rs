//! 组句的展示状态：缓冲变化时重查候选并重建 [`Composed`]，云端词异步并入，高亮 / 翻页，按状态生成给 DLL 的帧。

mod grid;
mod state;

#[cfg(test)]
mod tests;

use qingjian_core::candidate::arrows::Focus;
use qingjian_core::{
    Candidate, CandidateKind, CandidateLayout, CandidateList, Cell, CloudWord, Engine, Grid, Query,
    SurroundingText,
};
use qingjian_platform::protocol::{Frame, KeyEvent, KeyModifiers, PreeditKind, PreeditSegment};

pub(super) use self::state::Composed;
use super::Router;

impl Router {
    /// 缓冲变化后：按 Engine 状态重建 [`Composed`]，发一次云联想请求，归零高亮与整句补全。
    pub(super) fn recompose(&mut self, surrounding: Option<SurroundingText>) {
        self.highlight = 0;
        self.navigated = false;
        // 新一轮查询收回单行（与 macOS 壳、Windows Server 一致）
        self.grid = None;
        self.sentence = None;
        if self.engine.composition().is_empty() {
            self.focus = Focus::default();
            self.composed = None;
            self.engine.cancel_prediction();
            self.stop_rescoring();
            return;
        }
        self.attach_loaded_model();
        let built = self.engine.query().ok().map(|mut query| {
            let items = std::mem::take(&mut query.candidates.items);
            let (preedit, cursor) = marked_parts(&query);
            (items, preedit, cursor, Box::new(query))
        });
        self.composed = Some(match built {
            Some((items, preedit, cursor, spelling)) => {
                let layout =
                    CandidateLayout::new(items, self.config.page_size, self.config.cloud_slots);
                if self.config.cloud_enabled
                    && self.engine.prediction_enabled()
                    && self.prediction_context_active()
                {
                    self.engine.request_prediction(surrounding, layout.local());
                }
                Composed::Candidates {
                    preedit,
                    cursor,
                    layout,
                    spelling,
                    manual_sequence: None,
                }
            }
            None => {
                self.engine.cancel_prediction();
                let composition = self.engine.composition();
                let text = composition.text().to_owned();
                let cursor = text[..composition.cursor()].chars().count();
                Composed::Raw { text, cursor }
            }
        });
        self.highlight = (0..self.candidate_count())
            .find(|&index| self.layout_candidate(index).is_some())
            .unwrap_or(0);
        self.schedule_rescoring();
    }

    /// 已挂起的会话重新获得焦点时，异步请求已经作废；按当前本地候选补发。
    pub(super) fn request_current_prediction(&mut self) {
        if !self.config.cloud_enabled
            || !self.engine.prediction_enabled()
            || !self.prediction_context_active()
        {
            return;
        }
        if let Some(Composed::Candidates { layout, .. }) = &self.composed {
            self.engine.request_prediction(None, layout.local());
        }
    }

    /// 显式按需联想：跳过停键等待，用同一份应用上下文；记序号供结果布局匹配。
    /// 返回是否真的发出了请求。
    pub(super) fn request_manual_prediction(
        &mut self,
        surrounding: Option<SurroundingText>,
    ) -> bool {
        if !self.config.cloud_enabled || !self.engine.prediction_enabled() {
            return false;
        }
        let Some(Composed::Candidates {
            layout,
            manual_sequence,
            ..
        }) = self.composed.as_mut()
        else {
            return false;
        };
        *manual_sequence = self
            .engine
            .request_manual_prediction(surrounding, layout.local());
        manual_sequence.is_some()
    }

    /// 组句中按需联想的组合键（`[shortcut] predict`）。按字符匹配：`.` 的键码与 Delete 冲突。
    pub(super) fn matches_predict_combo(&self, event: &KeyEvent) -> bool {
        let combo = self.config.predict;
        event
            .character
            .is_some_and(|c| c.eq_ignore_ascii_case(&combo.key))
            && event.modifiers.chord() == KeyModifiers::from(combo.modifiers)
    }

    /// 只补第一页末尾尚未被用户选中的格；按需联想的结果置顶并入；整句补全仍可单独显示。
    pub(super) fn poll_prediction(&mut self) {
        if !self.prediction_context_active() {
            return;
        }
        let Some(prediction) = self.engine.poll_prediction() else {
            return;
        };
        if let Some(job) = self.translation.as_mut() {
            if let Some(text) = prediction.sentence {
                job.result = Some(text);
            } else {
                self.end_translation();
            }
            return;
        }
        let Some(Composed::Candidates {
            layout,
            manual_sequence,
            ..
        }) = self.composed.as_mut()
        else {
            return;
        };
        let manual = manual_sequence.take() == Some(prediction.sequence);
        self.sentence = prediction.sentence;
        if self.navigated || self.highlight >= self.config.page_size {
            return;
        }
        let words = prediction
            .words
            .into_iter()
            .map(CloudWord::into_candidate)
            .collect();
        if manual {
            layout.set_cloud_first(words);
        } else {
            layout.set_cloud(words);
        }
    }

    fn prediction_context_active(&self) -> bool {
        self.focused
            .and_then(|session| self.sessions.get(&session))
            .is_some_and(|info| info.active && !info.private)
    }

    /// 高亮移动 `delta`，夹在 `[0, 末尾]`，到页边自然换页。
    pub(super) fn move_highlight(&mut self, delta: isize) {
        if delta == 0 {
            return;
        }
        let count = self.candidate_count();
        if count == 0 {
            self.highlight = 0;
            return;
        }
        let mut next = self.highlight as isize + delta.signum();
        while (0..count as isize).contains(&next) {
            if self.layout_candidate(next as usize).is_some() {
                self.navigated = true;
                self.highlight = next as usize;
                break;
            }
            next += delta.signum();
        }
    }

    /// 整页翻 `step`，高亮落到目标页第一个候选；矩阵展开着时一次翻一屏。
    pub(super) fn page(&mut self, step: isize) {
        if step == 0 {
            return;
        }
        if self.grid.is_some() {
            if self.move_screens(step) {
                self.engine.note_page_turn();
            }
            return;
        }
        let count = self.candidate_count();
        if count == 0 {
            self.highlight = 0;
            return;
        }
        let page_size = self.config.page_size;
        let page_count = count.div_ceil(page_size);
        let current = (self.highlight / page_size) as isize;
        let mut target = (current + step).clamp(0, page_count as isize - 1);
        while (0..page_count as isize).contains(&target) {
            let start = target as usize * page_size;
            if let Some(index) = (start..(start + page_size).min(count))
                .find(|&index| self.layout_candidate(index).is_some())
            {
                if target != current {
                    self.navigated = true;
                    self.engine.note_page_turn();
                }
                self.highlight = index;
                break;
            }
            target += step.signum();
        }
    }

    /// 往 `step` 方向还有没有页可翻：`-` `=` 当翻页键时翻不动就落回原字符。
    pub(super) fn can_page(&self, step: isize) -> bool {
        let page_size = self.config.page_size;
        let page_count = self.candidate_count().div_ceil(page_size) as isize;
        let target = (self.highlight / page_size) as isize + step;
        (0..page_count).contains(&target)
    }

    pub(super) fn candidate_count(&self) -> usize {
        match &self.composed {
            Some(Composed::Candidates { layout, .. }) => layout.len(),
            _ => 0,
        }
    }

    /// 候选布局里第 `index` 个（跨页下标）。
    pub(super) fn layout_candidate(&self, index: usize) -> Option<Candidate> {
        match &self.composed {
            Some(Composed::Candidates { layout, .. }) => layout.candidate(index).cloned(),
            _ => None,
        }
    }

    pub(super) fn commit_index(&mut self, index: usize) -> Option<String> {
        let candidate = self.layout_candidate(index)?;
        Some(self.engine.commit(&candidate))
    }

    /// 按当前状态生成一帧：翻译评审优先；没在组句给空帧；否则给高亮所在的那一页。
    pub(super) fn current_frame(&self) -> Frame {
        if self.translation.is_some() {
            return self.translation_frame();
        }
        match &self.composed {
            None => Frame::default(),
            Some(Composed::Raw { text, cursor }) => Frame {
                preedit: vec![PreeditSegment {
                    text: text.clone(),
                    kind: PreeditKind::Typed,
                }],
                cursor: *cursor,
                window_preedit: None,
                preedit_mode: self.config.preedit,
                candidates: CandidateList { items: Vec::new() },
                highlight: usize::MAX,
                page: 0,
                page_count: 1,
                layout: self.config.layout,
                columns: 0,
                column_ems: Vec::new(),
                appearance: self.config.appearance,
                aux_code_show: false,
                translation_pos: self.config.translation_pos,
                sentence: None,
                notice: self.notice.clone(),
                symbol_panel: false,
            },
            Some(Composed::Candidates {
                preedit,
                cursor,
                layout,
                spelling,
                ..
            }) => {
                let page_size = self.config.page_size;
                let highlight = self.highlight.min(layout.len().saturating_sub(1));
                // 高亮候选的读音切法与首选不同（`ganga` 的 尴尬 / 刚啊）：拼音行按它重切
                let candidate = layout.candidate(highlight);
                let followed = (spelling.spelling_of(candidate) != spelling.spelled).then(|| {
                    let mut followed = (**spelling).clone();
                    followed.follow(candidate);
                    followed
                });
                let (preedit, cursor) = match &followed {
                    Some(followed) => marked_parts(followed),
                    None => (preedit.clone(), *cursor),
                };
                let window_preedit = window_parts(
                    &self.engine,
                    followed.as_ref().unwrap_or(spelling),
                    candidate,
                    &preedit,
                );
                let page = highlight / page_size;
                // 横排展开成矩阵时画视口里的几行（行尾补空位），序号由前端只标在高亮所在那一行；单行时只画当前页。
                // 空文本只用于显示占位，不传给 Engine 上屏；保持数字标签与固定位置一致。
                let viewport = self.grid.as_ref().map(|grid| grid.rows(layout));
                let columns = if viewport.is_some() { page_size } else { 0 };
                let first = viewport
                    .as_ref()
                    .map_or(page * page_size, |rows| rows.start * page_size);
                let items: Vec<Candidate> = viewport
                    .unwrap_or(page..page + 1)
                    .flat_map(|row| {
                        let mut cells = layout.page(row);
                        if columns > 0 {
                            cells.resize(page_size, Cell::Empty);
                        }
                        cells
                    })
                    .map(|cell| cell.candidate().cloned().unwrap_or_else(empty_cell))
                    .collect();
                let mut candidates = CandidateList { items };
                self.engine.annotate(&mut candidates);
                Frame {
                    preedit,
                    cursor,
                    window_preedit,
                    preedit_mode: self.config.preedit,
                    candidates,
                    // 方向键在拼音行里时候选不画高亮，空格照样上屏原来高亮的那个
                    highlight: if self.config.two_area_arrows() && self.focus == Focus::Pinyin {
                        usize::MAX
                    } else {
                        highlight - first
                    },
                    page,
                    page_count: layout.pages().max(1),
                    layout: self.config.layout,
                    columns,
                    column_ems: if columns > 0 {
                        Grid::column_ems(layout)
                    } else {
                        Vec::new()
                    },
                    appearance: self.config.appearance,
                    aux_code_show: false,
                    translation_pos: self.config.translation_pos,
                    sentence: self.sentence.clone(),
                    notice: self.notice.clone(),
                    symbol_panel: false,
                }
            }
        }
    }
}

/// 候选窗口里的拼音行：带调拼音开着、高亮候选读得出声调时换成它的读音，与行内拼音相同时不另给（同 Windows Server）。
fn window_parts(
    engine: &Engine,
    query: &Query,
    candidate: Option<&Candidate>,
    preedit: &[PreeditSegment],
) -> Option<Box<(Vec<PreeditSegment>, usize)>> {
    let toned = engine.toned_syllables(candidate?)?;
    let (segments, cursor) = query.window_segments(Some(&toned));
    let segments: Vec<PreeditSegment> = segments.iter().map(Into::into).collect();
    (segments != preedit).then(|| Box::new((segments, cursor)))
}

/// 帧里空位的占位候选（云端词还没到、矩阵行尾不满）：文本为空，前端按空格子画。
fn empty_cell() -> Candidate {
    Candidate {
        text: String::new(),
        kind: CandidateKind::Chinese,
        syllables: Vec::new(),
        reading: None,
        translation: None,
        aux_code: None,
    }
}

/// 一次查询的拼音行分段与光标。光标用 Core 的映射：自动补的 `'` 会让显示串比敲的长。
pub(super) fn marked_parts(query: &Query) -> (Vec<PreeditSegment>, usize) {
    (
        query.marked_segments().iter().map(Into::into).collect(),
        query.marked_cursor(),
    )
}
