//! 对着一个假 Server 跑客户端，外加回包解析。
//!
//! 假 Server 只复刻协议的形状（哪条消息有回包、回什么壳），不复刻引擎：
//! 这里要钉住的是**客户端有没有按协议说话**，真引擎的行为由 Server 自己的测试管
//! （`apps/linux/server/tests/client.rs` 拿同一个客户端对着真 Server 走）。

mod events;
mod reply;
mod session;

use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::thread;

use serde_json::{Value, json};

use crate::protocol::{Frame, KeyOutcome, read_message, write_message};

/// 假 Server 收到的消息，测完拿回来核对。
type Received = std::sync::Arc<std::sync::Mutex<Vec<Value>>>;

/// 起一个假 Server，返回 socket 路径与收到的消息。
///
/// `limit` 是「服务几条消息之后就收工关连接」，用来演 Server 中途没了；`None` 是一直服务到客户端断开。
fn fake_server(limit: Option<usize>) -> (PathBuf, Received, thread::JoinHandle<()>) {
    let path = std::env::temp_dir().join(format!(
        "qingjian-client-test-{}-{:?}.sock",
        std::process::id(),
        thread::current().id()
    ));
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).expect("绑定假 Server 的 socket");
    let received: Received = Default::default();
    let seen = received.clone();
    let handle = thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let mut served = 0;
        while let Ok(Some(value)) = read_message::<_, Value>(&mut stream) {
            seen.lock().expect("记下收到的消息").push(value.clone());
            served += 1;
            let reply = if value.get("OpenSession").is_some() {
                Some(json!({"Update": {"session": 1, "frame": Frame::default(), "linux_ui": {}}}))
            } else if value.get("LinuxHello").is_some() {
                Some(json!({"LinuxHello": {"session": 1, "page_size": 9}}))
            } else if let Some(request) = value.get("LinuxEvent") {
                Some(event_reply(&request["event"]))
            } else if value.get("Poll").is_some() {
                Some(json!({"Update": {
                    "session": 1,
                    "frame": Frame::default(),
                    "identity": {"generation": 1, "context": "/ibus/context/1", "revision": 8},
                }}))
            } else {
                // CloseSession 没有回包
                None
            };
            if let Some(reply) = reply
                && write_message(&mut stream, &reply).is_err()
            {
                return;
            }
            if limit.is_some_and(|limit| served >= limit) {
                return;
            }
        }
    });
    (path, received, handle)
}

/// 假 Server 对一个 Linux 事件的回包，形状照真 Server：问菜单回 `Menu`，报了能读选区的选区翻译键回 `RequestSelection`，
/// 其余回 `KeyResult`，上面另附展示身份与模式（轮询靠身份里的版本号判断帧换没换）。
fn event_reply(event: &Value) -> Value {
    if event.get("Menu").is_some() {
        return json!({"Menu": {"session": 1, "items": [
            {"id": 3, "label": "降权"},
            {"id": 5, "label": "删除"},
        ]}});
    }
    if event["Key"]["selection_supported"] == json!(true) {
        return json!({"RequestSelection": {"session": 1, "request": 4}});
    }
    json!({"KeyResult": {
        "session": 1,
        "outcome": KeyOutcome::Consumed,
        "commit": Value::Null,
        "frame": Frame::default(),
        "identity": {"generation": 1, "context": "/ibus/context/1", "revision": 7},
        "mode": {"english": false, "traditional": false, "full_width": true, "scheme": ""},
    }})
}
