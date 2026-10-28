//! 输入方案的装载：拼音侧（全拼、内置或用户双拼、注音）与形码侧按配置找出来挂到引擎上，与 Windows Server 的 `dispatch/schemes.rs` 对齐。
//!
//! 随包方案在随包根的 `assets/schemes/`，用户方案在用户数据目录的 `schemes/<id>/`，
//! 找法与「没变不重读」见 [`qingjian_platform::SchemeSlot`]。辅码方案也放在这两处，由 `code_tables::AuxCodeSources` 去找。

use std::path::{Path, PathBuf};

use qingjian_platform::Config;

use super::Router;

/// 随包的方案目录。走 `assets/` 而不是 `data/`：码表随 git 跟踪，装机与开发布局里是同一个相对路径（理由同 Windows Server）。
const BUNDLED_SCHEMES: &str = "assets/schemes";

/// 随包的方案目录；不在为 `None`。
pub fn find_bundled_schemes(root: &Path) -> Option<PathBuf> {
    let dir = root.join(BUNDLED_SCHEMES);
    dir.is_dir().then_some(dir)
}

impl Router {
    /// 启动时：记下去哪找方案（`user_dir` 是用户数据目录，方案在它的 `schemes/` 下），按当前配置装配一次，
    /// 再往输入日志里记这次启动的会话信息：里面有方案，回放照它装配引擎，所以要在装好之后记。
    pub fn configure_schemes(
        &mut self,
        config: &Config,
        user_dir: Option<PathBuf>,
        bundled_schemes: Option<PathBuf>,
    ) {
        self.schemes_user_dir = user_dir;
        self.bundled_schemes = bundled_schemes;
        self.reload_schemes(config);
        self.engine.log_session(env!("CARGO_PKG_VERSION"), "linux");
    }

    /// 启动日志里的方案名（单开全拼也写「全拼」）。
    pub fn scheme_name(&self) -> String {
        self.scheme_slot.scheme_name(self.config.scheme)
    }

    /// 回包 `mode.scheme` 的方案名，按状态条的写法：单开全拼是空串，用户双拼没装上时按全拼算。
    pub(super) fn scheme_label(&self) -> String {
        self.scheme_slot.scheme_label(self.config.scheme)
    }

    /// 按配置的两条轴（`[general] scheme` 与 `code`）装配引擎；形码方案与码表都没变时不重读码表。
    pub(super) fn reload_schemes(&mut self, config: &Config) {
        self.scheme_slot.apply(
            &mut self.engine,
            config,
            self.schemes_user_dir.as_deref(),
            self.bundled_schemes.as_deref(),
        );
        if self.composed.is_some() {
            self.recompose(None);
        }
    }
}
