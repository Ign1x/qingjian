//! `org.kde.StatusNotifierItem`：托盘里那个图标。左键（`Activate`）切中 / 英，右键由托盘宿主按 `Menu` 属性弹 dbusmenu 菜单。

use qingjian_platform::protocol::linux::InputMode;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::ObjectPath;

use super::MENU_PATH;
use super::clicks::Clicks;
use super::icons::{Icons, Picture};
use crate::session::ModeTarget;

pub struct Item {
    /// 现在的模式；`None` 是还不知道。
    mode: Option<InputMode>,

    clicks: Clicks,

    icons: Icons,
}

impl Item {
    pub fn new(mode: Option<InputMode>, clicks: Clicks, icons: Icons) -> Self {
        Self {
            mode,
            clicks,
            icons,
        }
    }

    /// 换上新模式，返回（图标变没变，提示变没变）。
    pub fn set(&mut self, mode: Option<InputMode>) -> (bool, bool) {
        let icon = |mode: Option<&InputMode>| mode.map(|mode| mode.english);
        let changed = (
            icon(self.mode.as_ref()) != icon(mode.as_ref()),
            describe(self.mode.as_ref()) != describe(mode.as_ref()),
        );
        self.mode = mode;
        changed
    }
}

/// 托盘提示的说明：「中文 · 小鹤双拼 · 全角标点 · 繁体输出」这样；还不知道模式时说一句什么时候会有。
pub fn describe(mode: Option<&InputMode>) -> String {
    let Some(mode) = mode else {
        return "进到输入框里就显示中 / 英".to_owned();
    };
    let mut parts = vec![if mode.english { "英文" } else { "中文" }];
    if !mode.english && !mode.scheme.is_empty() {
        parts.push(&mode.scheme);
    }
    parts.push(if mode.full_width {
        "全角标点"
    } else {
        "半角标点"
    });
    if mode.traditional {
        parts.push("繁体输出");
    }
    parts.join(" · ")
}

/// 属性大多一直不变；会变的（图标、提示）不发 `PropertiesChanged`，按规范发 `NewIcon` / `NewToolTip`，托盘宿主收到再来读。
#[zbus::interface(name = "org.kde.StatusNotifierItem")]
impl Item {
    #[zbus(property(emits_changed_signal = "const"))]
    fn category(&self) -> &str {
        "SystemServices"
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn id(&self) -> &str {
        "qingjian"
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn title(&self) -> &str {
        "青简"
    }

    /// waybar 缺省不显示 `Passive` 的图标。
    #[zbus(property(emits_changed_signal = "const"))]
    fn status(&self) -> &str {
        "Active"
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn window_id(&self) -> i32 {
        0
    }

    /// 不知道模式时用图标主题里的 `qingjian`（装好的 hicolor 那几张，与 Fcitx5 托盘一样）；知道了留空，托盘宿主改用 `IconPixmap`。
    #[zbus(property(emits_changed_signal = "false"))]
    fn icon_name(&self) -> &str {
        match self.mode {
            None => "qingjian",
            Some(_) => "",
        }
    }

    #[zbus(property(emits_changed_signal = "false"))]
    fn icon_pixmap(&self) -> Vec<Picture> {
        self.icons.get(self.mode.as_ref()).to_vec()
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn overlay_icon_name(&self) -> &str {
        ""
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn overlay_icon_pixmap(&self) -> Vec<Picture> {
        Vec::new()
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn attention_icon_name(&self) -> &str {
        ""
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn attention_icon_pixmap(&self) -> Vec<Picture> {
        Vec::new()
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn attention_movie_name(&self) -> &str {
        ""
    }

    /// （图标名，图标，标题，说明）：图标留空，托盘宿主用条目自己的。
    #[zbus(property(emits_changed_signal = "false"))]
    fn tool_tip(&self) -> (String, Vec<Picture>, String, String) {
        (
            String::new(),
            Vec::new(),
            "青简".to_owned(),
            describe(self.mode.as_ref()),
        )
    }

    /// 不是只有菜单：左键单击走 `Activate`（切中 / 英），菜单留给右键。
    #[zbus(property(emits_changed_signal = "const"))]
    fn item_is_menu(&self) -> bool {
        false
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn menu(&self) -> ObjectPath<'_> {
        ObjectPath::from_static_str_unchecked(MENU_PATH)
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn icon_theme_path(&self) -> &str {
        ""
    }

    /// 左键单击：切中 / 英，与 Windows 任务栏上点「中 / 英」一样。按托盘上画着的反过来要（不知道时当中文）。
    fn activate(&self, _x: i32, _y: i32) {
        let english = self.mode.as_ref().is_some_and(|mode| mode.english);
        self.clicks.send(ModeTarget::English(!english));
    }

    /// 中键：不做事。
    fn secondary_activate(&self, _x: i32, _y: i32) {}

    /// 不支持 dbusmenu 的托盘宿主要条目自己弹菜单：没有界面库弹不了，不做事。
    fn context_menu(&self, _x: i32, _y: i32) {}

    fn scroll(&self, _delta: i32, _orientation: &str) {}

    #[zbus(signal)]
    pub(super) async fn new_icon(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    pub(super) async fn new_tool_tip(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}
