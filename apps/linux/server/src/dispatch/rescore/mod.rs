//! 本地整句模型（与 Windows Server 的 `dispatch/rescore` 对齐）：后台加载、停键后请求重排、结果到了重建当前布局。
//!
//! 按键回调里永远只跑词级模型；模型的意见在停键 80 毫秒后请求、几十毫秒后到，更新候选顺序，
//! 用户翻过页或动过高亮就不打扰。前文优先用前端随按键带来的应用光标前文（`linux.rs` 里交给 `set_rescoring_context`），应用不报时用本会话最近上屏的字。
//! 节拍由主循环驱动：[`Router::next_tick`] 说下次多久来一次 [`Router::tick`]；插件组句期间每 80 毫秒的 `Poll` 也顺带 tick。
//! 模型在后台接上时用户正在组句，这一轮也补一次重排。目录解析与加载在 [`qingjian_platform::models`]，进行态在 [`state`]。

mod state;

use std::time::Duration;

use qingjian_core::CandidateLayout;
use qingjian_core::sentence::SentenceScorer;
use qingjian_platform::LocalModelConfig;

pub(crate) use self::state::RescoreState;
use super::Router;
use super::composed::{Composed, marked_parts};
use qingjian_platform::models::{Loaded, LoadedModel, ModelDirs, ModelLoader};

/// 空闲时主循环多久醒一次：到点落盘学习数据。
const IDLE_TICK: Duration = Duration::from_secs(1);

impl Router {
    /// 启动时：记下去哪找模型，按 `[model] enabled` 决定要不要加载。
    pub fn configure_local_model(&mut self, dirs: ModelDirs, config: &LocalModelConfig) {
        self.model_dirs = dirs;
        self.applied_model = config.clone();
        self.apply_choice_tier();
        if config.enabled {
            self.load_local_model();
        }
    }

    /// 热加载：开关或选的模型变了才重载 / 卸载，档位每次都套。
    pub(super) fn apply_model_config(&mut self, config: &LocalModelConfig) {
        let changed =
            (config.enabled, config.name) != (self.applied_model.enabled, self.applied_model.name);
        self.applied_model = config.clone();
        self.apply_choice_tier();
        if !changed {
            return;
        }
        self.unload_local_model();
        if config.enabled {
            self.load_local_model();
        }
    }

    fn apply_choice_tier(&mut self) {
        self.engine
            .set_choice_tier(self.applied_model.zhiyi_tier.choice_tier());
    }

    /// 在后台线程加载选的模型；没有模型文件就什么都不做。
    fn load_local_model(&mut self) {
        if self.model_loader.is_some()
            || self.engine.has_sentence_scorer()
            || self.engine.has_choice_model()
        {
            return;
        }
        let Some(files) = self.model_dirs.resolve(self.applied_model.name) else {
            tracing::info!("没有本地模型文件，不重排");
            return;
        };
        self.model_loader = ModelLoader::spawn(files);
    }

    /// 卸掉模型（配置关掉或换了模型）。
    fn unload_local_model(&mut self) {
        self.model_loader = None;
        self.engine.set_async_sentence_scorer(None);
        self.engine.set_choice_model(None);
        self.rescore.stop();
    }

    /// 加载线程有结果了就接到 Engine 上；每次按键 / tick 顺手看一眼，不阻塞。
    pub(super) fn attach_loaded_model(&mut self) {
        let Some(loader) = &self.model_loader else {
            return;
        };
        match loader.poll() {
            Loaded::Pending => {}
            Loaded::Done(result) => {
                self.model_loader = None;
                match result {
                    Ok(model) => self.attach_model(model),
                    Err(error) => tracing::warn!(%error, "本地模型加载失败，不重排"),
                }
            }
            Loaded::Gone => self.model_loader = None,
        }
    }

    fn attach_model(&mut self, model: LoadedModel) {
        match model {
            LoadedModel::Sentence(scorer) => self.engine.set_async_sentence_scorer(Some(scorer)),
            LoadedModel::Choice(model) => self.engine.set_choice_model(Some(model)),
        }
        self.engine.log_session(env!("CARGO_PKG_VERSION"), "linux");
        self.rescore_current_round();
    }

    /// 打分器接到 Engine 上（后台加载完成，或测试直接注入）：日志补一条会话信息，正在组句就补这一轮的重排。
    pub fn attach_sentence_scorer(&mut self, scorer: Box<dyn SentenceScorer>) {
        self.engine.set_async_sentence_scorer(Some(scorer));
        // 模型上线了：日志里补一条会话信息，之后的条目知道重排开着
        self.engine.log_session(env!("CARGO_PKG_VERSION"), "linux");
        self.rescore_current_round();
    }

    /// 模型接上时用户正在组句：这一轮的查询从没见过打分器，不补查一次就永远错过重排。
    /// 补查只为攒下整句路径、起防抖，不动画面；用户已翻页或动过高亮就不打扰。
    fn rescore_current_round(&mut self) {
        if !matches!(self.composed, Some(Composed::Candidates { .. })) || !self.on_first_page() {
            return;
        }
        if self.engine.query().is_ok() {
            tracing::info!("模型接上时正在组句，补一轮重排");
            self.schedule_rescoring();
        }
    }

    /// 组句结束 / 换会话：什么都不等了。
    pub(super) fn stop_rescoring(&mut self) {
        self.rescore.stop();
    }

    /// 缓冲变化之后：有整句路径等着打分就起防抖计时，否则停下。
    pub(super) fn schedule_rescoring(&mut self) {
        if self.engine.rescoring_pending() {
            self.rescore.schedule();
        } else {
            self.rescore.stop();
        }
    }

    /// 主循环下次该多久后来一次 [`Router::tick`]：在等重排就按它的节拍，否则按落盘学习的一秒。
    pub fn next_tick(&self) -> Duration {
        self.rescore
            .next_deadline()
            .map_or(IDLE_TICK, |deadline| deadline.min(IDLE_TICK))
    }

    /// 防抖到点就发请求；在等结果就收一次，收到了重查并重建当前布局。
    pub(super) fn advance_rescoring(&mut self) {
        if self.engine.composition().is_empty() {
            self.rescore.stop();
            return;
        }
        if self.rescore.debounce_elapsed() {
            if self.engine.request_rescoring() {
                self.rescore.start_polling();
            } else {
                self.rescore.stop();
            }
        }
        if !self.rescore.polling() {
            return;
        }
        if !self.engine.poll_rescoring() {
            // 等太久多半是前文变了（上屏后接着打下一段）、结果作废；真卡住也只是这轮不重排
            if self.rescore.expired() {
                tracing::debug!("等本地整句模型超时，本轮不重排");
                self.rescore.stop();
            }
            return;
        }
        self.rescore.stop();
        if self.on_first_page() {
            self.requery_rescored();
        }
    }

    /// 用户还看着第一页、没动过高亮：模型的结果才换正在看的这页。
    fn on_first_page(&self) -> bool {
        self.highlight < self.config.page_size && !self.navigated
    }

    /// 分回来了：按重排后的顺序重建候选布局，下一次 `Poll` 回的帧就是新顺序。
    fn requery_rescored(&mut self) {
        let Ok(mut query) = self.engine.query() else {
            return;
        };
        let Some(Composed::Candidates {
            preedit,
            cursor,
            layout,
            spelling,
            ..
        }) = self.composed.as_mut()
        else {
            return;
        };
        let cloud = layout.cloud().to_vec();
        *layout = CandidateLayout::new(
            std::mem::take(&mut query.candidates.items),
            self.config.page_size,
            self.config.cloud_slots,
        );
        layout.set_cloud(cloud);
        (*preedit, *cursor) = marked_parts(&query);
        **spelling = query;
        // 这次查询可能又记下了一批要打分的（缓存按前文记，前文没变时不会），再来一轮
        self.schedule_rescoring();
    }
}

#[cfg(test)]
mod tests;
