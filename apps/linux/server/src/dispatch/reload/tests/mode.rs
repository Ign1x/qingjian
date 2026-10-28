//! 按住切换键期间热加载配置，旧按下不能在新策略下补切换。

use super::{poll, temp_dir, watched, write_config};
use qingjian_platform::protocol::{
    ClientMessage, KeyEvent, KeyModifiers, PROTOCOL_VERSION, SessionId,
};
use serde_json::json;

#[test]
fn mode_reload_cancels_pending_modifier_taps() {
    for (name, source, expected_english) in [
        ("tap-key", "[shortcut]\nswitch_mode = \"none\"\n", false),
        (
            "tap-policy",
            "[general]\ndefault_english = true\n[shortcut]\nswitch_mode = \"shift\"\n",
            true,
        ),
    ] {
        let dir = temp_dir(name);
        let mut router = watched(&dir, "[shortcut]\nswitch_mode = \"shift\"\n");
        router.handle(ClientMessage::OpenSession {
            session: SessionId(1),
            app: None,
            protocol: PROTOCOL_VERSION,
        });
        router.handle_linux(
            json!({"LinuxEvent": {"session": 1, "event": {"Capabilities": {
                "password": false, "sensitive": false, "disabled": false
            }}}}),
        );
        let event = KeyEvent::new(0x10, None, KeyModifiers::default());
        router.handle_linux(json!({"LinuxEvent": {"session": 1, "event": {"Key": {"event": event, "release": false}}}}));
        assert!(router.sessions[&SessionId(1)].switch_tap.is_some());
        write_config(&dir.join("config.toml"), source, 200);
        poll(&mut router);
        let released = router.handle_linux(json!({"LinuxEvent": {"session": 1, "event": {"Key": {"event": event, "release": true}}}})).unwrap();
        assert_eq!(released["KeyResult"]["outcome"], "Passthrough");
        assert_eq!(released["KeyResult"]["mode"]["english"], expected_english);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
