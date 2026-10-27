//! 右键菜单的回包：只有问菜单（`LinuxEvent::Menu`）才会收到，单独解析，不混进 [`super::Reply`]——
//! 它不是一帧，当成按键结果会被前端当空帧收起候选窗。

use serde_json::Value;

use super::ClientError;
use crate::protocol::linux::{MenuItem, MenuReply};

/// 这一格能做的几项；Server 回 `Ignored`（问的那一帧已经过期、会话不在了）时为空，空的就不弹菜单。
pub(super) fn menu_items(value: Value) -> Result<Vec<MenuItem>, ClientError> {
    if value.get("Ignored").is_some() {
        return Ok(Vec::new());
    }
    value
        .get("Menu")
        .and_then(|body| serde_json::from_value::<MenuReply>(body.clone()).ok())
        .map(|menu| menu.items)
        .ok_or_else(|| ClientError::Unexpected(value.to_string()))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn items_ignored_and_garbage() {
        let items = menu_items(json!({"Menu": {"session": 1, "items": [
            {"id": 1, "label": "置顶"},
            {"id": 5, "label": "删除"},
        ]}}))
        .expect("菜单回包");
        assert_eq!(
            items,
            [
                MenuItem {
                    id: 1,
                    label: "置顶".to_owned()
                },
                MenuItem {
                    id: 5,
                    label: "删除".to_owned()
                },
            ]
        );
        assert!(
            menu_items(json!({"Ignored": {"session": 1}}))
                .expect("过期")
                .is_empty()
        );
        assert!(matches!(
            menu_items(json!({"Menu": {"session": 1}})),
            Err(ClientError::Unexpected(_))
        ));
        assert!(matches!(
            menu_items(json!({"KeyResult": {}})),
            Err(ClientError::Unexpected(_))
        ));
    }
}
