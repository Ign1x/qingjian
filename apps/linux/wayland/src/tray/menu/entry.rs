//! 菜单里的一项：画成什么、点了做什么，以及它在 dbusmenu 里的属性。

use std::collections::HashMap;

use zbus::zvariant::{OwnedValue, Str};

use super::action::Action;
use super::kind::Kind;

/// dbusmenu 一项的属性（`a{sv}`）。
pub type Properties = HashMap<String, OwnedValue>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub label: String,

    pub kind: Kind,

    /// 点了做什么；分隔线与灰着的字是 `None`。
    pub action: Option<Action>,
}

impl Entry {
    pub fn new(label: impl Into<String>, kind: Kind, action: Option<Action>) -> Self {
        Self {
            label: label.into(),
            kind,
            action,
        }
    }

    pub fn separator() -> Self {
        Self::new("", Kind::Separator, None)
    }

    /// 给 dbusmenu 的属性，缺省值（`type` 为 standard、`enabled` 为真）不写；`names` 不空时只给列出来的那几样。
    pub fn properties(&self, names: &[String]) -> Properties {
        let mut properties = Properties::new();
        let mut toggle = |kind: &str, on: bool| {
            properties.insert("toggle-type".to_owned(), text(kind));
            properties.insert("toggle-state".to_owned(), OwnedValue::from(i32::from(on)));
        };
        match self.kind {
            Kind::Radio(on) => toggle("radio", on),
            Kind::Check(on) => toggle("checkmark", on),
            Kind::Plain => {}
            Kind::Note => {
                properties.insert("enabled".to_owned(), OwnedValue::from(false));
            }
            Kind::Separator => {
                properties.insert("type".to_owned(), text("separator"));
            }
        }
        if self.kind != Kind::Separator {
            properties.insert("label".to_owned(), text(&self.label));
        }
        filter(properties, names)
    }
}

/// 只留 `names` 里有的；`names` 空着是全要。
pub fn filter(mut properties: Properties, names: &[String]) -> Properties {
    if !names.is_empty() {
        properties.retain(|name, _| names.contains(name));
    }
    properties
}

pub fn text(value: &str) -> OwnedValue {
    OwnedValue::from(Str::from(value.to_owned()))
}
