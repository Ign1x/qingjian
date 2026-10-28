//! 配置变化时矩阵、挂起会话与旧面板身份的边界。

use super::{poll, temp_dir, watched, write_config};
use crate::dispatch::Router;
use qingjian_core::CustomPhrase;
use qingjian_platform::protocol::{
    ClientMessage, KeyEvent, KeyModifiers, PROTOCOL_VERSION, SessionId,
};
use serde_json::{Value, json};

fn event(router: &mut Router, id: u64, event: Value) -> Value {
    router
        .handle_linux(json!({"LinuxEvent": {"session": id, "event": event}}))
        .unwrap()
}

fn press(router: &mut Router, code: u32, character: Option<char>) -> Value {
    event(
        router,
        1,
        json!({"Key": {
            "event": KeyEvent::new(code, character, KeyModifiers::default()), "release": false
        }}),
    )
}

fn setup(name: &str) -> (Router, std::path::PathBuf) {
    let dir = temp_dir(name);
    let mut router = watched(
        &dir,
        "[general]\npage_size = 1\nlayout = \"horizontal\"\nhorizontal_grid = true\n",
    );
    router
        .engine
        .set_custom_phrases(
            (1..=9)
                .map(|n| CustomPhrase::new("qq", &format!("第{n}项")))
                .collect(),
        )
        .unwrap();
    for id in [1, 2] {
        router.handle(ClientMessage::OpenSession {
            session: SessionId(id),
            app: None,
            protocol: PROTOCOL_VERSION,
        });
        router.handle_linux(json!({"DisplayReporting": {"session": id,
            "identity": {"generation": 1, "context": format!("grid-{id}"), "revision": 0},
            "features": {"grid": true}
        }}));
        event(
            &mut router,
            id,
            json!({"Capabilities": {"password": false, "disabled": false, "sensitive": false}}),
        );
    }
    event(&mut router, 1, json!({"Focus": {"focused": true}}));
    for c in "qq".chars() {
        press(&mut router, c as u32, Some(c));
    }
    for _ in 0..7 {
        press(&mut router, 0x28, None);
    }
    assert!(router.grid.unwrap().top() > 0);
    (router, dir)
}

#[test]
fn changing_page_size_rebuilds_active_and_suspended_grids() {
    for suspended in [false, true] {
        let (mut router, dir) = setup(if suspended {
            "grid-suspended"
        } else {
            "grid-active"
        });
        if suspended {
            router.ensure_focus(SessionId(2));
        }
        write_config(
            &dir.join("config.toml"),
            "[general]\npage_size = 3\nlayout = \"horizontal\"\nhorizontal_grid = true\n",
            200,
        );
        poll(&mut router);
        router.ensure_focus(SessionId(1));
        let frame = router.current_frame();
        assert_eq!(frame.columns, 0, "换分页后先收起旧矩阵");
        assert_eq!(frame.candidates.items.len(), 3);
        assert_eq!(frame.candidates.items[frame.highlight].text, "第8项");
        assert_eq!(
            press(&mut router, 0x31, Some('1'))["KeyResult"]["commit"],
            "第7项"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn collapsing_grid_on_reload_rejects_clicks_from_the_old_viewport() {
    let (mut router, dir) = setup("grid-identity");
    let old = press(&mut router, 0x28, None)["KeyResult"]["identity"].clone();
    write_config(
        &dir.join("config.toml"),
        "[general]\npage_size = 1\nlayout = \"horizontal\"\nhorizontal_grid = false\n",
        200,
    );
    poll(&mut router);
    let response = event(
        &mut router,
        1,
        json!({"Candidate": {"identity": old, "index": 0}}),
    );
    assert!(
        response.get("Ignored").is_some(),
        "旧矩阵第零格不能变成当前页第零格"
    );
    assert_eq!(router.engine.composition().text(), "qq");
    std::fs::remove_dir_all(dir).unwrap();
}
