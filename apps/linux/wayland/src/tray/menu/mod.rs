//! `com.canonical.dbusmenu`：托盘图标的右键菜单（托盘宿主照着画原生菜单）。
//!
//! 中文 / 英文（二选一）、全角标点、繁体输出（勾选态照现在的模式）、设置…，最后一行灰字写版本。
//! 不放「退出」：退出后这个 seat 上就没有输入法了，systemd 单元对正常退出不重启，误点了只能去终端拉起来。
//! 菜单是平的，一项的编号是它在 [`entries`] 里的位置加一，0 是根。

mod action;
mod entry;
mod kind;
mod service;

pub use action::Action;
pub use entry::{Entry, Properties};
pub use kind::Kind;
pub use service::Menu;

use std::collections::HashMap;

use qingjian_platform::protocol::linux::InputMode;
use zbus::zvariant::{OwnedValue, Value};

use crate::session::ModeTarget;
use entry::{filter, text};

/// dbusmenu 的一个节点 `(ia{sv}av)`：编号、属性、子节点（每个子节点也是这样一个节点，包在 variant 里）。
pub type Node = (i32, Properties, Vec<OwnedValue>);

/// 这个模式下的菜单。不知道模式时中 / 英两项都不选、两个开关都不勾，照样能点（会话先问 Server 再切）。
pub fn entries(mode: Option<&InputMode>) -> Vec<Entry> {
    let english = mode.is_some_and(|mode| mode.english);
    let full_width = mode.is_some_and(|mode| mode.full_width);
    let traditional = mode.is_some_and(|mode| mode.traditional);
    vec![
        Entry::new(
            "中文",
            Kind::Radio(mode.is_some() && !english),
            Some(Action::Mode(ModeTarget::English(false))),
        ),
        Entry::new(
            "英文",
            Kind::Radio(english),
            Some(Action::Mode(ModeTarget::English(true))),
        ),
        Entry::separator(),
        Entry::new(
            "全角标点",
            Kind::Check(full_width),
            Some(Action::Mode(ModeTarget::FullWidth(!full_width))),
        ),
        Entry::new(
            "繁体输出",
            Kind::Check(traditional),
            Some(Action::Mode(ModeTarget::Traditional(!traditional))),
        ),
        Entry::separator(),
        Entry::new("设置…", Kind::Plain, Some(Action::Settings)),
        Entry::separator(),
        Entry::new(
            format!("青简 Wayland 原生版 {}", env!("CARGO_PKG_VERSION")),
            Kind::Note,
            None,
        ),
    ]
}

/// 从 `parent` 起的布局：根带上各项（`depth` 为 0 时不带），某一项就是它自己（菜单是平的，没有子节点）；没有这一项为 `None`。
pub fn layout(entries: &[Entry], parent: i32, depth: i32, names: &[String]) -> Option<Node> {
    if parent == 0 {
        let children = if depth == 0 {
            Vec::new()
        } else {
            (1..)
                .zip(entries)
                .map(|(id, entry)| boxed((id, entry.properties(names), Vec::new())))
                .collect()
        };
        return Some((0, root(names), children));
    }
    let entry = entries.get(index(parent)?)?;
    Some((parent, entry.properties(names), Vec::new()))
}

/// 根的属性：说明它下面是一个菜单。
fn root(names: &[String]) -> Properties {
    filter(
        HashMap::from([("children-display".to_owned(), text("submenu"))]),
        names,
    )
}

/// 子节点放进 `av`：包成 variant。值里没有文件描述符，转成自有的不会失败。
fn boxed(node: Node) -> OwnedValue {
    OwnedValue::try_from(Value::from(node)).expect("菜单节点里没有文件描述符")
}

/// 编号 → [`entries`] 里的位置；根与越界的编号为 `None`。
fn index(id: i32) -> Option<usize> {
    usize::try_from(id).ok()?.checked_sub(1)
}
