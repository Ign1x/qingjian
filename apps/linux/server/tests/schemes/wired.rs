//! 真的 Server 进程：随包根下的 `assets/schemes` 装得上，回包报真正在用的方案名。

use std::os::unix::net::UnixStream;

use qingjian_platform::protocol::{PROTOCOL_VERSION, read_message, write_message};
use serde_json::{Value, json};

use super::repository;
use super::server::Server;

fn exchange(stream: &mut UnixStream, message: Value) -> Value {
    write_message(stream, &message).unwrap();
    read_message::<_, Value>(stream)
        .unwrap()
        .expect("连接被断开")
}

#[test]
fn the_server_finds_bundled_schemes_under_its_resources() {
    let directory =
        std::env::temp_dir().join(format!("qingjian-schemes-wired-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    let config = directory.join("config/qingjian/config.toml");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, "[general]\nscheme = \"none\"\ncode = \"wubi86\"\n").unwrap();
    // 随包根的布局与装好之后一样：`resources/assets/schemes/wubi86`
    let bundled = directory.join("resources/assets/schemes/wubi86");
    std::fs::create_dir_all(&bundled).unwrap();
    for name in ["scheme.toml", "table.tsv"] {
        std::fs::copy(
            repository().join("assets/schemes/wubi86").join(name),
            bundled.join(name),
        )
        .unwrap();
    }
    let mut server = Server::start(directory.clone());
    let mut stream = server.connect();
    for message in [
        json!({"OpenSession": {"session": 1, "app": null, "protocol": PROTOCOL_VERSION}}),
        json!({"LinuxHello": {"version": 4, "session": 1, "generation": 1, "context": "schemes-wired"}}),
        json!({"LinuxEvent": {"session": 1, "event": {"Capabilities": {"sensitive": false, "password": false, "disabled": false}}}}),
    ] {
        exchange(&mut stream, message);
    }
    let mut result = Value::Null;
    for c in "ggll".chars() {
        result = exchange(
            &mut stream,
            json!({"LinuxEvent": {"session": 1, "event": {"Key": {"release": false, "event": {
                "virtual_key": c.to_ascii_uppercase() as u32, "character": c,
                "modifiers": {"ctrl": false, "shift": false, "alt": false, "win": false, "caps": false, "english_mode": false}}}}}}),
        );
    }
    let reply = &result["KeyResult"];
    assert_eq!(
        reply["frame"]["candidates"]["items"][0]["text"], "一",
        "{reply}"
    );
    assert_eq!(reply["mode"]["scheme"], "五笔（86）");
    drop(stream);
    drop(server);
    let _ = std::fs::remove_dir_all(&directory);
}
