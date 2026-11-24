//! 配置热加载：空闲时看 `config.toml` 的 mtime，改了就重读并应用（与 Windows Server 的 `dispatch/reload` 对齐）。
//! 只重设启动时会应用的那些项；便宜的无条件重设，本地模型 / 释义表 / 附加词库 / 辅码码表按配置变化重建，
//! 输入方案每次按配置重新装配（码表没变不重读），用户 `dicts/`、`codes/`、`schemes/` 与 `code_dicts/` 目录的文件增删也跟着重装。
//! 拼音显示位置在会话握手时交给插件，改了要重启服务。

mod cloud;
mod display;
mod state;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use qingjian_core::{Language, NoTranslator};
use qingjian_platform::{Config, UserDirs};
use qingjian_predict::API_USAGE_FILE;

pub(crate) use self::state::ConfigReload;
use super::{Router, RouterConfig};
use crate::assembly;

/// 看配置文件 mtime 的最短间隔。
const CONFIG_POLL_INTERVAL: Duration = Duration::from_secs(1);

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
}

impl Router {
    /// 开启热加载：记下路径、数据目录与当前已应用的 dictionaries / model / 学习语言。
    pub fn watch_config(
        &mut self,
        config: &Config,
        config_path: PathBuf,
        root: PathBuf,
        user: impl Into<UserDirs>,
    ) {
        let user = user.into();
        self.apply_schemes(config, &root, &user.custom());
        let mut reload = ConfigReload {
            applied_config: config.clone(),
            scheme_files: Vec::new(),
            last_mtime: mtime(&config_path),
            config_path,
            last_check: Instant::now(),
            root,
            user,
            dictionary_files: Vec::new(),
            glossary_files: Vec::new(),
            applied_dictionaries: config.dictionaries.clone(),
            applied_model: config.model.clone(),
            applied_predict: config.predict.clone(),
            applied_language: assembly::learning_language(config),
            applied_aux_code: config.aux_code.clone(),
            code_files: Vec::new(),
            code_word_files: Vec::new(),
        };
        reload.dictionary_files = reload.dict_snapshot();
        reload.code_files = reload.code_snapshot();
        reload.code_word_files = reload.code_word_snapshot();
        reload.glossary_files = reload.glossary_snapshot();
        reload.scheme_files = reload.scheme_snapshot();
        self.reload = Some(reload);
    }

    /// `config.toml` 路径；没开热加载（测试）时为 `None`。托盘切换与候选菜单往里写，热加载再读回来。
    pub(super) fn config_path(&self) -> Option<&Path> {
        self.reload
            .as_ref()
            .map(|reload| reload.config_path.as_path())
    }

    /// `tick` 里调；一秒内只真正看一次。解析失败保持原配置，mtime 照记（不每秒重试同一个坏文件）。
    pub(super) fn poll_config_reload(&mut self) {
        let Some(reload) = &mut self.reload else {
            return;
        };
        if reload.last_check.elapsed() < CONFIG_POLL_INTERVAL {
            return;
        }
        reload.last_check = Instant::now();
        let files = reload.dict_snapshot();
        if files != reload.dictionary_files {
            self.engine
                .set_extra_dictionaries(reload.load_dictionaries());
            reload.dictionary_files = files;
        }
        // 设置页下载装上（或更新了）释义表：英→中表与配置里的学习语言都按新文件重装
        let glossaries = reload.glossary_snapshot();
        if glossaries != reload.glossary_files {
            reload.glossary_files = glossaries;
            self.reload_glossaries();
        }
        let Some(reload) = &mut self.reload else {
            return;
        };
        // 用户 `codes/` 或 `schemes/` 下的文件增删或更新（设置页刚导入 / 移除一张码表、改了辅码方案）：不必等配置改动
        let codes_changed = reload.code_snapshot() != reload.code_files;
        if codes_changed {
            self.reload_aux_codes();
        }
        let Some(reload) = &mut self.reload else {
            return;
        };
        let words = reload.code_word_snapshot();
        let words_changed = words != reload.code_word_files;
        reload.code_word_files = words;
        let current = mtime(&reload.config_path);
        let changed = current != reload.last_mtime;
        reload.last_mtime = current;
        let old_scheme_files = reload.scheme_files.clone();
        let files = reload.scheme_snapshot();
        let schemes_changed = files != reload.scheme_files;
        reload.scheme_files = files;
        let reset_for_bundled_scheme =
            schemes_changed && reload.selected_bundled_scheme_changed(&old_scheme_files);
        if !changed && !schemes_changed && !codes_changed && !words_changed {
            return;
        }
        let path = reload.config_path.clone();
        let applied = reload.applied_config.clone();
        let root = reload.root.clone();
        let user_dir = reload.user.custom();
        if changed {
            match Config::load(&path) {
                Ok(config) => {
                    self.apply_config(&config);
                    tracing::info!("configuration reloaded");
                    return;
                }
                Err(error) => {
                    tracing::error!(%error, "failed to reload configuration, keeping previous configuration")
                }
            }
        }
        self.apply_schemes(&applied, &root, &user_dir);
        if words_changed || reset_for_bundled_scheme {
            self.reset_all_inputs();
        }
    }

    /// 应用新配置，项目与 `main.rs` 启动时设的一致。
    fn apply_config(&mut self, config: &Config) {
        let aux_view_changed = self.config.aux_code_show != config.general.aux_code_show
            || self.engine.aux_code_key() != config.general.aux_code_key();
        self.engine.set_fuzzy(config.fuzzy.clone());
        self.engine
            .set_shuangpin_raw_preedit(config.general.shuangpin_raw_preedit);
        self.engine
            .set_shuangpin_full_pinyin(config.general.shuangpin_full_pinyin);
        self.engine
            .set_hold_selections(config.general.hold_selections);
        self.engine.set_show_tones(config.general.show_tones);
        self.engine
            .set_context_memory(config.model.context_memory_seconds);
        self.engine
            .set_shift_letter_compose(config.general.shift_letter.compose());
        self.engine.set_learning(config.general.learning);
        self.engine
            .set_auto_phrase(config.auto_phrase.enabled, config.auto_phrase.max_chars);
        self.engine.set_chinese_first(config.general.chinese_first);
        self.engine
            .set_spelling_correction(config.general.spelling_correction);
        self.engine
            .set_english_in_chinese(config.general.english_in_chinese);
        self.engine
            .set_english_max_in_chinese(config.general.english_max_in_chinese());
        self.engine
            .set_english_min_position(config.general.english_min_position());
        self.engine
            .set_emoji_limit(usize::from(config.general.emoji.get()));
        self.engine
            .set_kaomoji_limit(usize::from(config.general.kaomoji.get()));
        self.engine
            .set_symbol_limit(usize::from(config.general.symbols.get()));
        self.engine.set_traditional(config.general.traditional());
        self.engine
            .set_half_width_symbols(&config.general.half_width_symbols);
        self.engine.set_mode_keys(config.shortcut.mode);
        self.engine.set_auto_space(config.general.auto_space);
        self.engine
            .set_aux_code_key(config.general.aux_code_key(), config.general.page_keys());
        self.engine
            .set_aux_keep_empty(config.general.aux_code_keep_empty);
        self.engine.set_aux_enabled(config.aux_code.enabled);
        self.engine.set_aux_show(config.general.aux_code_show);
        if let Err(error) = self
            .engine
            .set_custom_phrases(config.custom_phrases.clone())
        {
            tracing::warn!(%error, "invalid custom phrases, keeping previous phrases");
        }
        let next = RouterConfig::from(config);
        self.reload_mode(&next);
        self.config = next;
        self.config.page_size = self.config.page_size.clamp(1, 9);
        self.reload_display();

        let Some(mut reload) = self.reload.take() else {
            return;
        };
        let schemes_changed = self.apply_schemes(config, &reload.root, &reload.user.custom());
        reload.applied_config = config.clone();
        if config.predict != reload.applied_predict {
            let usage_path = Some(reload.user.records().join(API_USAGE_FILE));
            self.reload_cloud(&config.predict, usage_path);
            reload.applied_predict = config.predict.clone();
        }
        if !self.engine.prediction_enabled() {
            self.config.cloud_slots = 0;
        }
        if config.model != reload.applied_model {
            self.apply_model_config(&config.model);
            reload.applied_model = config.model.clone();
        }
        let language = assembly::learning_language(config);
        if language != reload.applied_language && self.swap_translator(language, &reload) {
            reload.applied_language = language;
        }
        if config.dictionaries != reload.applied_dictionaries {
            reload.applied_dictionaries = config.dictionaries.clone();
            self.engine
                .set_extra_dictionaries(reload.load_dictionaries());
            self.engine
                .set_blocked_words(config.dictionaries.blocked.clone());
            self.engine.set_base_words(config.dictionaries.base);
        }
        let aux_changed = config.aux_code != reload.applied_aux_code;
        reload.applied_aux_code = config.aux_code.clone();
        self.reload = Some(reload);
        if schemes_changed {
            self.reset_all_inputs();
        }
        if aux_changed {
            self.reload_aux_codes();
        } else if aux_view_changed {
            self.refresh_input(false);
        }
    }

    /// 下载的释义表变了：英→中表重装；学习语言按当前配置重新换一次表（之前缺表没换成的这次能换上）。
    fn reload_glossaries(&mut self) {
        let Some(mut reload) = self.reload.take() else {
            return;
        };
        // 下载的删了、又没随包的：英文候选右侧留空
        let english = assembly::glossary_file(
            &reload.root,
            Some(&reload.user.downloads()),
            Language::Chinese,
        )
        .and_then(|path| assembly::load_english_glossary(&path));
        match english {
            Some(glossary) => self.engine.set_english_translator(Box::new(glossary)),
            None => self.engine.set_english_translator(Box::new(NoTranslator)),
        }
        let language = match Config::load(&reload.config_path) {
            Ok(config) => assembly::learning_language(&config),
            Err(_) => reload.applied_language,
        };
        if self.swap_translator(language, &reload) {
            reload.applied_language = language;
        }
        self.reload = Some(reload);
    }

    /// 按已应用的 `[aux_code]` 重装辅码码表：`[aux_code]` 变了或用户 `codes/` / `schemes/` 下的文件变了都走这里。
    /// Core 换表时退出辅码态，组句中就按新表重查一次，免得回包的帧还是旧表筛出来的。
    fn reload_aux_codes(&mut self) {
        let Some(reload) = &mut self.reload else {
            return;
        };
        let tables = reload.aux_sources().load(&reload.applied_aux_code);
        reload.code_files = reload.code_snapshot();
        tracing::info!(count = tables.len(), "auxiliary code tables reloaded");
        self.engine.set_aux_codes(tables);
        self.refresh_input(true);
    }

    /// 学习语言变了就换释义表：关是不翻译；没有这门语言的表或装不上就保持原样。换成功（或关掉）返回 true。
    fn swap_translator(&mut self, language: Option<Language>, reload: &ConfigReload) -> bool {
        let Some(language) = language else {
            self.engine.set_translator(Box::new(NoTranslator));
            tracing::info!("学习语言已关，不显示译文");
            return true;
        };
        let Some(path) =
            assembly::glossary_file(&reload.root, Some(&reload.user.downloads()), language)
        else {
            tracing::warn!(
                language = language.code(),
                "没有这门语言的释义表，学习语言不变"
            );
            return false;
        };
        match assembly::load_glossary(language, &path, Some(&reload.user.learning())) {
            Ok(glossary) => {
                tracing::info!(language = language.code(), "释义表已切换");
                self.engine.set_translator(Box::new(glossary));
                true
            }
            Err(error) => {
                tracing::warn!(%error, "释义表加载失败，学习语言不变");
                false
            }
        }
    }
}
