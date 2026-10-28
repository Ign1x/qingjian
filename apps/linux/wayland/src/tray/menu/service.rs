//! 托盘菜单的 D-Bus 对象：响应宿主读取与点击，模式变化时通知布局更新。

use qingjian_platform::protocol::linux::InputMode;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::OwnedValue;

use super::action::Action;
use super::entry::Properties;
use super::{Node, entries, index, layout};
use crate::tray::clicks::Clicks;
use crate::tray::settings::Launcher;

pub struct Menu {
    /// 现在的模式；`None` 是还不知道。
    mode: Option<InputMode>,

    /// 布局的版本号，菜单变了加一（`LayoutUpdated` 带着它）。
    revision: u32,

    clicks: Clicks,

    settings: Launcher,
}

impl Menu {
    pub fn new(mode: Option<InputMode>, clicks: Clicks) -> Self {
        Self {
            mode,
            revision: 1,
            clicks,
            settings: Launcher::default(),
        }
    }

    /// 换上新模式；菜单因此变了就加版本号并返回它（发 `LayoutUpdated` 用）。
    pub fn set(&mut self, mode: Option<InputMode>) -> Option<u32> {
        let changed = entries(self.mode.as_ref()) != entries(mode.as_ref());
        self.mode = mode;
        changed.then(|| {
            self.revision = self.revision.wrapping_add(1);
            self.revision
        })
    }

    /// 点了第 `id` 项。返回认不认得这一项。
    fn click(&self, id: i32) -> bool {
        let entries = entries(self.mode.as_ref());
        let Some(entry) = index(id).and_then(|index| entries.get(index)) else {
            return false;
        };
        match entry.action {
            Some(Action::Mode(target)) => self.clicks.send(target),
            Some(Action::Settings) => self.settings.open(),
            None => {}
        }
        true
    }
}

#[zbus::interface(name = "com.canonical.dbusmenu")]
impl Menu {
    #[zbus(property(emits_changed_signal = "const"))]
    fn version(&self) -> u32 {
        3
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn text_direction(&self) -> &str {
        "ltr"
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn status(&self) -> &str {
        "normal"
    }

    #[zbus(property(emits_changed_signal = "const"))]
    fn icon_theme_path(&self) -> Vec<String> {
        Vec::new()
    }

    /// `recursion_depth` 为 -1 是全要；菜单是平的，大于 0 都一样。
    fn get_layout(
        &self,
        parent_id: i32,
        recursion_depth: i32,
        property_names: Vec<String>,
    ) -> zbus::fdo::Result<(u32, Node)> {
        let entries = entries(self.mode.as_ref());
        let node = layout(&entries, parent_id, recursion_depth, &property_names)
            .ok_or_else(|| zbus::fdo::Error::InvalidArgs(format!("no menu item {parent_id}")))?;
        Ok((self.revision, node))
    }

    /// `ids` 空着是全部。
    fn get_group_properties(
        &self,
        ids: Vec<i32>,
        property_names: Vec<String>,
    ) -> Vec<(i32, Properties)> {
        let entries = entries(self.mode.as_ref());
        let ids = if ids.is_empty() {
            (0..=entries.len() as i32).collect()
        } else {
            ids
        };
        ids.into_iter()
            .filter_map(|id| {
                let (id, properties, _) = layout(&entries, id, 0, &property_names)?;
                Some((id, properties))
            })
            .collect()
    }

    fn get_property(&self, id: i32, name: String) -> zbus::fdo::Result<OwnedValue> {
        let entries = entries(self.mode.as_ref());
        let names = [name];
        layout(&entries, id, 0, &names)
            .and_then(|(_, mut properties, _)| properties.remove(&names[0]))
            .ok_or_else(|| zbus::fdo::Error::InvalidArgs(format!("no property {}", names[0])))
    }

    /// 只认 `clicked`；`opened` / `closed` / `hovered` 什么都不用做。
    fn event(
        &self,
        id: i32,
        event_id: String,
        _data: OwnedValue,
        _timestamp: u32,
    ) -> zbus::fdo::Result<()> {
        if event_id == "clicked" && !self.click(id) {
            return Err(zbus::fdo::Error::InvalidArgs(format!("no menu item {id}")));
        }
        Ok(())
    }

    /// 回认不得的编号。
    fn event_group(&self, events: Vec<(i32, String, OwnedValue, u32)>) -> Vec<i32> {
        events
            .into_iter()
            .filter(|(id, event_id, _, _)| event_id == "clicked" && !self.click(*id))
            .map(|(id, ..)| id)
            .collect()
    }

    /// 菜单在每次模式变了时已经发过 `LayoutUpdated`，弹出前不用再更新。
    fn about_to_show(&self, _id: i32) -> bool {
        false
    }

    fn about_to_show_group(&self, _ids: Vec<i32>) -> (Vec<i32>, Vec<i32>) {
        (Vec::new(), Vec::new())
    }

    #[zbus(signal)]
    pub(in crate::tray) async fn layout_updated(
        emitter: &SignalEmitter<'_>,
        revision: u32,
        parent: i32,
    ) -> zbus::Result<()>;
}
