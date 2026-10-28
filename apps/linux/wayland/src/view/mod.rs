//! 候选窗的绘制：协议帧按配置里的外观（「跟随系统」时按桌面的偏好）画成预乘 RGBA 位图，
//! 再按 popup 的缩放裁边、换成 Wayland 的像素格式（[`blit`]）。右键菜单另画一张、叠在候选窗上合成一张（[`Canvas`]），
//! 鼠标落点也按它认。贴到 surface 上是 `crate::wayland` 的事；设计见 `docs/design/linux-wayland.md`「显示」。

mod bitmap;
mod canvas;
mod look;
mod menu;
mod mode;
mod status;

#[cfg(test)]
mod tests;

pub use bitmap::{Placement, blit, paste};
pub use canvas::Canvas;
pub use menu::Menu;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use qingjian_platform::protocol::Frame;
use qingjian_platform::{Appearance, GeneralConfig, display};
use qingjian_render::{
    FontLibrary, HitRegion, HitTarget, Layout, Mode, RenderError, Rendered, Renderer, Theme,
    ThemeLibrary, UiFont, sample, system_fonts,
};

use look::Look;

use crate::portal::{Desktop, Preference};

/// 字体库按哪种语言挑汉字字形。
const LOCALE: &str = "zh-CN";

/// 一次最多回报几个义项：多了 Server 断开连接。
const MAX_SENSES: usize = 128;

/// 候选窗的渲染器与它用的外观；一个进程一份，popup 隐藏时只 [`Painter::forget`]。
pub struct Painter {
    /// 渲染器（字体库随它），换字体时重建。
    renderer: Renderer,

    /// 上一帧的点击区域，译词已换成 Core 的义项编号（[`renumber_senses`]）；动画帧布局不变，照抄。
    hits: Vec<HitRegion>,

    /// 主题（浅色那一份，画的时候按帧里的外观取深浅，「跟随系统」看桌面），已套上设置里的字号与动画开关。
    theme: Theme,

    /// 在用的外观设置。
    look: Look,

    config_path: PathBuf,

    /// 用户主题目录；`None` 只有内置主题。
    themes_dir: Option<PathBuf>,

    /// 上次读配置时配置文件的戳（[`Look::stamp`]）。
    config_stamp: u64,

    /// 上次读主题时用户主题目录的戳（[`ThemeLibrary::stamp`]）。
    themes_stamp: u64,

    /// 桌面的外观偏好（portal 的后台线程更新）：「跟随系统」按它取深浅，减少动态效果跟着它。
    desktop: Arc<Desktop>,

    /// 上次画或查的时候桌面的偏好，每秒和现在的比一次。
    seen: Preference,

    /// 上一帧的外观设置：只有「跟随系统」的帧要跟着桌面换深浅。
    appearance: Appearance,

    /// 画右键菜单的渲染器（`menu/paint.rs`）：第一次弹菜单时才建，建不成为 `Some(None)`。
    menu_renderer: Option<Option<Renderer>>,

    /// 输入模式（Server 回包上的），主题里的 `mode.*` 照它画；还没收到时是缺省（中文）。
    mode: Mode,
}

impl Painter {
    /// 读配置、建字体库与渲染器，再画一遍样例帧预热。系统里一个界面字体都没有（字体库建不起来）时为 `None`。
    pub fn new(
        config_path: PathBuf,
        themes_dir: Option<PathBuf>,
        desktop: Arc<Desktop>,
    ) -> Option<Self> {
        let started = Instant::now();
        // 先取戳再读：读的途中文件又改了，下一次 reload 还能看出来
        let config_stamp = Look::stamp(&config_path);
        let themes_stamp = themes_stamp(themes_dir.as_deref());
        let look = Look::read(&config_path).unwrap_or_else(|error| {
            tracing::warn!(%error, "配置文件读不了，候选窗按缺省外观画");
            Look::new(&GeneralConfig::default())
        });
        let library = font_library(&look.font)
            .inspect_err(|error| tracing::warn!(%error, "渲染器字体库加载失败，候选窗画不出来"))
            .ok()?;
        let font = library.ui_family().to_owned();
        let mut painter = Self {
            renderer: Renderer::new(library),
            hits: Vec::new(),
            theme: look.theme(themes_dir.as_deref()),
            look,
            config_path,
            themes_dir,
            config_stamp,
            themes_stamp,
            seen: desktop.preference(),
            desktop,
            appearance: Appearance::default(),
            menu_renderer: None,
            mode: Mode::default(),
        };
        painter
            .renderer
            .load_theme_fonts(&painter.theme, system_fonts::family_files);
        painter.warm_up();
        tracing::info!(
            elapsed = ?started.elapsed(),
            font,
            theme = painter.look.theme_id,
            "候选窗使用青简渲染器"
        );
        Some(painter)
    }

    /// 按 `scale`（点 → 像素的倍数，如 1.5）画一帧；出错记日志返回 `None`。
    pub fn render(&mut self, frame: &Frame, scale: f32) -> Option<Rendered> {
        let started = Instant::now();
        self.seen = self.desktop.preference();
        self.appearance = frame.appearance;
        self.renderer.set_reduce_motion(self.seen.reduce_motion);
        let dark = self.seen.dark_for(frame.appearance);
        let mut rendered = self
            .renderer
            .render(
                &display::render_frame(frame, self.mode.clone()),
                display::layout(frame.layout),
                &self.theme.with_dark(dark),
                scale,
            )
            .inspect_err(|error| tracing::warn!(%error, "候选窗渲染失败"))
            .ok()?;
        renumber_senses(frame, &mut rendered.hits);
        self.hits.clone_from(&rendered.hits);
        tracing::debug!(
            elapsed = ?started.elapsed(),
            width = rendered.pixmap.width(),
            height = rendered.pixmap.height(),
            "候选窗位图已画"
        );
        Some(rendered)
    }

    /// 动画的下一帧；没有在播的或出错时为 `None`。
    pub fn tick(&mut self) -> Option<Rendered> {
        let mut rendered = self
            .renderer
            .tick()
            .inspect_err(|error| tracing::warn!(%error, "候选窗动画帧渲染失败"))
            .ok()
            .flatten()?;
        rendered.hits.clone_from(&self.hits);
        Some(rendered)
    }

    /// popup 藏起来了：忘掉上一帧，下次出现不从旧位置过渡、循环动画从头播。
    pub fn forget(&mut self) {
        self.renderer.forget();
    }

    /// 每秒调一次，返回 `true` 时调用方重画当前帧：外观设置真变了（或主题文件改了），
    /// 或桌面的偏好变得让画着的那一帧不一样了（「跟随系统」换了深浅、减少动态效果开关了）。
    pub fn reload(&mut self) -> bool {
        let desktop = self.desktop.preference();
        let redraw = desktop.redraws(self.seen, self.appearance);
        self.seen = desktop;
        self.reread() || redraw
    }

    /// 配置文件与用户主题目录的戳都没变时只花几次 stat；变了就重读，
    /// 外观真变了（或主题文件改了）就换上新主题、字体变了还重建渲染器，返回 `true`。
    fn reread(&mut self) -> bool {
        let config_stamp = Look::stamp(&self.config_path);
        let themes_stamp = themes_stamp(self.themes_dir.as_deref());
        if config_stamp == self.config_stamp && themes_stamp == self.themes_stamp {
            return false;
        }
        // 主题 id 没变、主题文件本身改了也要重读，边改主题边看效果
        let themes_changed = themes_stamp != self.themes_stamp;
        self.config_stamp = config_stamp;
        self.themes_stamp = themes_stamp;
        let mut look = Look::read(&self.config_path).unwrap_or_else(|error| {
            // 改到一半存下的坏文件：外观保持不变，等下一次改动（与 Server 的热加载一样）
            tracing::warn!(%error, "配置文件读不了，候选窗外观保持不变");
            self.look.clone()
        });
        if look == self.look && !themes_changed {
            return false;
        }
        let mut rebuilt = false;
        if look.font != self.look.font {
            match font_library(&look.font) {
                Ok(library) => {
                    tracing::info!(font = library.ui_family(), "候选窗换了界面字体");
                    self.renderer = Renderer::new(library);
                    rebuilt = true;
                }
                Err(error) => {
                    // 记成没换，下次配置改动再试
                    tracing::warn!(%error, "界面字体加载失败，沿用原来的字体");
                    look.font.clone_from(&self.look.font);
                }
            }
        }
        self.look = look;
        self.theme = self.look.theme(self.themes_dir.as_deref());
        self.renderer
            .load_theme_fonts(&self.theme, system_fonts::family_files);
        if rebuilt {
            self.warm_up();
        }
        tracing::info!(theme = self.look.theme_id, "候选窗外观已重读");
        true
    }

    /// 画一遍样例帧再忘掉：字体与排版的缓存先热起来，第一次按键不用等。
    fn warm_up(&mut self) {
        if let Err(error) =
            self.renderer
                .render(&sample::nihao(), Layout::Vertical, &self.theme, 1.0)
        {
            tracing::warn!(%error, "候选窗预热渲染失败");
        }
        self.renderer.forget();
    }
}

/// 这一帧真正画出来的译词义项：`(候选在本页的位置, 义项编号)`，回报给 Server 让生词标记衰减；最多 128 对。
pub fn senses(rendered: &Rendered) -> Vec<(usize, usize)> {
    rendered
        .hits
        .iter()
        .filter_map(|region| match region.target {
            HitTarget::Translation { candidate, sense } => Some((candidate, sense)),
            HitTarget::Candidate(_) | HitTarget::MenuItem(_) => None,
        })
        .take(MAX_SENSES)
        .collect()
}

/// 渲染器按注记里的分隔符给译词编号，`display::from_candidate` 拼在最前面的读音（emoji 跟着的词、拆字的拼音）也占一号，
/// 只有读音、没有译文的候选也有。换成 Core 的义项编号并去掉不是译词的：回报里有一对对不上，Server 就丢掉整份。
fn renumber_senses(frame: &Frame, hits: &mut Vec<HitRegion>) {
    hits.retain_mut(|region| {
        let HitTarget::Translation { candidate, sense } = &mut region.target else {
            return true;
        };
        let Some(item) = frame
            .candidates
            .items
            .get(*candidate)
            .filter(|item| !item.text.is_empty())
        else {
            return false;
        };
        let count = item
            .translation
            .as_ref()
            .map_or(0, |translation| translation.senses().len());
        match sense.checked_sub(usize::from(item.reading.is_some())) {
            Some(core) if core < count => {
                *sense = core;
                true
            }
            _ => false,
        }
    });
}

/// `[general] font` 空为系统字体；指定的字族没装时字体库自己退回系统字体（记警告）。
fn font_library(font: &str) -> Result<FontLibrary, RenderError> {
    if font.is_empty() {
        return FontLibrary::system(LOCALE);
    }
    let ui_font = UiFont {
        family: font.to_owned(),
        files: system_fonts::family_files(font),
    };
    FontLibrary::with_ui_font(LOCALE, &ui_font)
}

/// 用户主题目录的戳；没有目录为 0。
fn themes_stamp(dir: Option<&Path>) -> u64 {
    dir.map_or(0, ThemeLibrary::stamp)
}
