//! 菜单的布局与勾选态、点了做什么、给 dbusmenu 的属性，以及托盘提示的说明。

use qingjian_platform::protocol::linux::InputMode;
use zbus::zvariant::{OwnedValue, Structure, Value};

use super::super::item::describe;
use super::super::menu::{Action, Entry, Kind, entries, layout};
use crate::session::ModeTarget;

fn text(value: &OwnedValue) -> &str {
    <&str>::try_from(value).expect("字符串")
}

fn kinds(menu: &[Entry]) -> Vec<Kind> {
    menu.iter().map(|entry| entry.kind).collect()
}

/// 中文 / 英文、全角标点、繁体输出、设置，最后一行灰字写版本；没有「退出」。
#[test]
fn the_menu_holds_the_switches_settings_and_version() {
    let menu = entries(None);
    let labels: Vec<_> = menu.iter().map(|entry| entry.label.as_str()).collect();
    assert_eq!(
        labels[..8],
        ["中文", "英文", "", "全角标点", "繁体输出", "", "设置…", ""]
    );
    assert_eq!(labels.len(), 9);
    assert!(
        labels[8].starts_with("青简 Wayland 原生版 "),
        "{}",
        labels[8]
    );
    assert_eq!(menu[8].kind, Kind::Note);
    assert_eq!(menu[8].action, None);
    assert_eq!(menu[6].action, Some(Action::Settings));
}

/// 勾选态照模式；还不知道模式时中 / 英两项都不选、两个开关都不勾。
#[test]
fn checks_follow_the_mode() {
    let unknown = kinds(&entries(None));
    assert_eq!(
        unknown[..5],
        [
            Kind::Radio(false),
            Kind::Radio(false),
            Kind::Separator,
            Kind::Check(false),
            Kind::Check(false)
        ]
    );
    let chinese = kinds(&entries(Some(&InputMode {
        full_width: true,
        ..InputMode::default()
    })));
    assert_eq!(
        chinese[..5],
        [
            Kind::Radio(true),
            Kind::Radio(false),
            Kind::Separator,
            Kind::Check(true),
            Kind::Check(false)
        ]
    );
    let english = kinds(&entries(Some(&InputMode {
        english: true,
        traditional: true,
        ..InputMode::default()
    })));
    assert_eq!(
        english[..5],
        [
            Kind::Radio(false),
            Kind::Radio(true),
            Kind::Separator,
            Kind::Check(false),
            Kind::Check(true)
        ]
    );
}

/// 点了要的是目标：中文 / 英文是哪边就是哪边，两个开关是画着的反过来。
#[test]
fn clicks_ask_for_targets() {
    let menu = entries(Some(&InputMode {
        full_width: true,
        ..InputMode::default()
    }));
    let actions: Vec<_> = menu.iter().map(|entry| entry.action).collect();
    assert_eq!(
        actions[..5],
        [
            Some(Action::Mode(ModeTarget::English(false))),
            Some(Action::Mode(ModeTarget::English(true))),
            None,
            Some(Action::Mode(ModeTarget::FullWidth(false))),
            Some(Action::Mode(ModeTarget::Traditional(true))),
        ]
    );
}

/// dbusmenu 的属性：二选一与开关带 toggle-type / toggle-state，分隔线只有 type，灰字是 enabled 假；能只取几样。
#[test]
fn properties_follow_dbusmenu() {
    let menu = entries(Some(&InputMode::default()));
    let radio = menu[0].properties(&[]);
    assert_eq!(text(&radio["label"]), "中文");
    assert_eq!(text(&radio["toggle-type"]), "radio");
    assert_eq!(i32::try_from(&radio["toggle-state"]), Ok(1));
    assert!(!radio.contains_key("type") && !radio.contains_key("enabled"));
    let check = menu[4].properties(&[]);
    assert_eq!(text(&check["toggle-type"]), "checkmark");
    assert_eq!(i32::try_from(&check["toggle-state"]), Ok(0));
    let separator = menu[2].properties(&[]);
    assert_eq!(separator.len(), 1);
    assert_eq!(text(&separator["type"]), "separator");
    let note = menu[8].properties(&[]);
    assert_eq!(bool::try_from(&note["enabled"]), Ok(false));
    let settings = menu[6].properties(&[]);
    assert_eq!(settings.len(), 1, "设置只有标题");
    let only = menu[3].properties(&["toggle-state".to_owned()]);
    assert_eq!(only.keys().collect::<Vec<_>>(), ["toggle-state"]);
}

/// 根下挂着各项（`(ia{sv}av)` 包在 variant 里，编号从 1 起），深度 0 不带；某一项就是它自己；没有的编号为空。
#[test]
fn the_layout_hangs_every_item_under_the_root() {
    let menu = entries(None);
    let (id, root, children) = layout(&menu, 0, -1, &[]).expect("根");
    assert_eq!(id, 0);
    assert_eq!(text(&root["children-display"]), "submenu");
    assert_eq!(children.len(), menu.len());
    for (index, child) in children.iter().enumerate() {
        assert_eq!(child.value_signature().to_string(), "(ia{sv}av)");
        let structure = <&Structure<'_>>::try_from(child).expect("子节点是结构");
        assert_eq!(structure.fields()[0], Value::I32(index as i32 + 1));
    }
    let (_, _, children) = layout(&menu, 0, 0, &[]).expect("根");
    assert!(children.is_empty(), "深度 0 不带子节点");

    let (id, properties, children) = layout(&menu, 4, -1, &[]).expect("第 4 项");
    assert_eq!(id, 4);
    assert_eq!(text(&properties["label"]), "全角标点");
    assert!(children.is_empty());
    assert!(layout(&menu, 10, -1, &[]).is_none());
    assert!(layout(&menu, -1, -1, &[]).is_none());
}

/// 托盘提示：中 / 英、方案（只在中文时）、标点、繁体；还不知道模式时说什么时候会有。
#[test]
fn the_tooltip_describes_the_mode() {
    assert_eq!(describe(None), "进到输入框里就显示中 / 英");
    assert_eq!(describe(Some(&InputMode::default())), "中文 · 半角标点");
    let shuangpin = InputMode {
        full_width: true,
        traditional: true,
        scheme: "小鹤双拼".to_owned(),
        ..InputMode::default()
    };
    assert_eq!(
        describe(Some(&shuangpin)),
        "中文 · 小鹤双拼 · 全角标点 · 繁体输出"
    );
    let english = InputMode {
        english: true,
        ..shuangpin
    };
    assert_eq!(describe(Some(&english)), "英文 · 全角标点 · 繁体输出");
}
