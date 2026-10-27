//! 候选右键菜单（`LinuxEvent::Menu` / `MenuPick`）：问这一格能做哪几件事、做点中的那一项，
//! 与 Windows Server 的 `dispatch/key/menu.rs` 同一套（`qingjian_platform::candidate_menu`）；
//! 做完的那句提示随回包的帧下发（`notice`，下一次按键清）。

use std::path::Path;

use qingjian_core::{Candidate, CustomPhrase};
use qingjian_platform::Config;
use qingjian_platform::candidate_menu::{self, CandidateAction};
use qingjian_platform::protocol::linux::MenuItem;

use super::Router;

impl Router {
    /// 当前页第 `index` 格（从 0 起）能做的几项；那一格没有候选（或不在组句、在翻译评审）时为空。
    pub(super) fn menu_items(&self, index: usize) -> Vec<MenuItem> {
        let Some(candidate) = self.menu_candidate(index) else {
            return Vec::new();
        };
        let Some(phrases) = self.configured_phrases() else {
            return Vec::new();
        };
        candidate_menu::actions(&self.engine, &candidate, &phrases)
            .into_iter()
            .map(|action| MenuItem {
                id: action.id(),
                label: action.label().to_owned(),
            })
            .collect()
    }

    /// 做菜单里编号 `id` 的那一项，做了返回 `true`。这一格此刻已经给不出这一项（弹菜单之后配置或学习数据变了）就不做。
    pub(super) fn pick_menu_item(&mut self, index: usize, id: u32) -> bool {
        let Some(action) = CandidateAction::from_id(id) else {
            return false;
        };
        let Some(candidate) = self.menu_candidate(index) else {
            return false;
        };
        let Some(phrases) = self.configured_phrases() else {
            return false;
        };
        if !candidate_menu::actions(&self.engine, &candidate, &phrases).contains(&action) {
            return false;
        }
        let path = self.config_path().map(Path::to_path_buf);
        let outcome = candidate_menu::perform(
            action,
            &mut self.engine,
            &candidate,
            path.as_deref(),
            &phrases,
        );
        // 不等热加载那一秒：短语当场换上，下面重排就按新的来
        if outcome.phrases_changed
            && let Some(phrases) = self.configured_phrases()
            && let Err(error) = self.engine.set_custom_phrases(phrases)
        {
            tracing::warn!(%error, "置顶后的自定义短语未应用");
        }
        tracing::info!(?action, "候选右键菜单");
        self.recompose(None);
        self.notice = Some(outcome.notice);
        true
    }

    /// 当前页第 `index` 格的候选（布局里的原样，不带译文）。
    fn menu_candidate(&self, index: usize) -> Option<Candidate> {
        let page_size = self.config.page_size;
        if self.translation.is_some() || self.engine.composition().is_empty() || index >= page_size
        {
            return None;
        }
        self.layout_candidate(self.highlight / page_size * page_size + index)
    }

    /// 配置文件里的自定义短语；没开热加载（测试）时为空表。
    /// 配置文件读不了（改到一半存下的坏文件）是 `None`：这时不能弹菜单——置顶是「读出短语表、加一条、整表写回」，
    /// 拿空表当底子写回去会把用户已有的自定义短语全部抹掉。
    fn configured_phrases(&self) -> Option<Vec<CustomPhrase>> {
        let Some(path) = self.config_path() else {
            return Some(Vec::new());
        };
        match Config::load(path) {
            Ok(config) => Some(config.custom_phrases),
            Err(error) => {
                tracing::warn!(%error, "配置文件读不了，这次不弹候选菜单");
                None
            }
        }
    }
}
