//! 真的 Server 进程：按 `[aux_code]` 从用户目录装上码表；同一条连接上插件（握手 v3）与 Rust 前端（v4）各收各的帧。

use std::os::unix::net::UnixStream;

use qingjian_dictionary::AuxCodeTable;
use qingjian_platform::protocol::{PROTOCOL_VERSION, read_message, write_message};
use serde_json::{Value, json};

use super::server::Server;

fn exchange(stream: &mut UnixStream, message: Value) -> Value {
    write_message(stream, &message).unwrap();
    read_message::<_, Value>(stream)
        .unwrap()
        .expect("连接被断开")
}

fn type_text(stream: &mut UnixStream, session: u64, text: &str) -> Value {
    let mut result = Value::Null;
    for c in text.chars() {
        let code = if c == ';' {
            0xBA
        } else {
            c.to_ascii_uppercase() as u32
        };
        result = exchange(
            stream,
            json!({"LinuxEvent": {"session": session, "event": {"Key": {"release": false, "event": {
                "virtual_key": code, "character": c,
                "modifiers": {"ctrl": false, "shift": false, "alt": false, "win": false, "caps": false, "english_mode": false}}}}}}),
        );
    }
    result["KeyResult"]["frame"].clone()
}

#[test]
fn the_server_loads_codes_and_downgrades_frames_for_plugins() {
    let directory = std::env::temp_dir().join(format!("qingjian-aux-wired-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    let config = directory.join("config/qingjian/config.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(
        &config,
        "[aux_code]\nenabled = true\n\n[[custom_phrases]]\ncode = \"kaifa\"\ntext = \"开发者大会\"\n",
    )
    .unwrap();
    let codes = directory.join("data/qingjian/codes");
    std::fs::create_dir_all(&codes).unwrap();
    AuxCodeTable::from_pairs([("开发".to_owned(), "kf".to_owned())])
        .unwrap()
        .write_qj(&codes.join("mine.qj"), &Default::default())
        .unwrap();
    let mut server = Server::start(directory.clone());
    let mut stream = server.connect();
    for (session, version) in [(1, 3), (2, 4)] {
        exchange(
            &mut stream,
            json!({"OpenSession": {"session": session, "app": null, "protocol": PROTOCOL_VERSION}}),
        );
        exchange(
            &mut stream,
            json!({"LinuxHello": {"version": version, "session": session, "generation": 1, "context": format!("wired-{session}")}}),
        );
        exchange(
            &mut stream,
            json!({"LinuxEvent": {"session": session, "event": {"Capabilities": {"sensitive": false, "password": false, "disabled": false}}}}),
        );
    }
    for (session, plugin) in [(1, true), (2, false)] {
        let typed = type_text(&mut stream, session, "kaifa");
        let items = typed["candidates"]["items"].as_array().unwrap();
        assert!(
            items.iter().any(|item| item["text"] == "开发者大会"),
            "{typed}"
        );
        assert_eq!(
            items.iter().any(|item| item["kind"].is_object()),
            !plugin,
            "会话 {session}：{typed}"
        );
        let coded = type_text(&mut stream, session, ";k");
        assert_eq!(coded["candidates"]["items"][0]["text"], "开发", "{coded}");
        let code_segment = coded["preedit"]
            .as_array()
            .unwrap()
            .iter()
            .any(|segment| segment["kind"] == "AuxCode");
        assert_eq!(code_segment, !plugin, "会话 {session}：{coded}");
    }
    drop(stream);
    drop(server);
    let _ = std::fs::remove_dir_all(&directory);
}
