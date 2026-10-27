//! 真实连接处理器与 Router 的资源回收；不增加产品诊断消息。
use super::{Request, connection::serve_connection};
use crate::dispatch::{Router, RouterConfig};
use qingjian_core::Engine;
use qingjian_dictionary::Dictionary;
use qingjian_platform::protocol::linux::{LINUX_UI_PROTOCOL, LINUX_UI_PROTOCOL_MIN};
use qingjian_platform::protocol::{PROTOCOL_VERSION, read_message, write_message};
use serde_json::{Value, json};
use std::os::unix::net::UnixStream;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::thread;
use std::time::Duration;

#[test]
fn close_and_disconnect_reclaim_every_router_session() {
    let (sender, receiver) = mpsc::sync_channel::<Request>(128);
    let count = Arc::new(AtomicUsize::new(0));
    let observed = count.clone();
    let worker = thread::spawn(move || {
        let mut router = Router::new(
            Engine::new(Dictionary::parse("你\tni\t100\n").unwrap()),
            RouterConfig::default(),
        );
        for (message, reply) in receiver {
            let response = router.handle_linux(message);
            observed.store(router.session_count(), Ordering::SeqCst);
            let _ = reply.send(response);
        }
        assert_eq!(router.session_count(), 0);
    });
    let (mut client, server) = UnixStream::pair().unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let connection = thread::spawn(move || serve_connection(server, sender, json!({"version": 3})));
    let open = |client: &mut UnixStream, id| {
        write_message(
            client,
            &json!({"OpenSession": {"session": id, "app": null, "protocol": PROTOCOL_VERSION}}),
        )
        .unwrap();
        assert_eq!(
            read_message::<_, Value>(client).unwrap().unwrap()["Update"]["session"],
            id
        );
    };
    for id in 1..=128 {
        open(&mut client, id);
    }
    assert_eq!(count.load(Ordering::SeqCst), 128);
    for id in 1..=128 {
        write_message(&mut client, &json!({"CloseSession": {"session": id}})).unwrap();
    }
    open(&mut client, 129); // 所有无回包通知已完成的顺序屏障。
    assert_eq!(count.load(Ordering::SeqCst), 1);
    for id in 130..=256 {
        open(&mut client, id);
    }
    drop(client);
    connection.join().unwrap();
    worker.join().unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 0);
}

/// 一条连接处理器，背后是 Router 线程；返回客户端那头。客户端断开后两个线程都收工。
fn serve() -> (UnixStream, thread::JoinHandle<()>, thread::JoinHandle<()>) {
    let (sender, receiver) = mpsc::sync_channel::<Request>(128);
    let worker = thread::spawn(move || {
        let mut router = Router::new(
            Engine::new(Dictionary::parse("你\tni\t100\n").unwrap()),
            RouterConfig::default(),
        );
        for (message, reply) in receiver {
            let _ = reply.send(router.handle_linux(message));
        }
    });
    let (client, server) = UnixStream::pair().unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let settings = json!({"version": LINUX_UI_PROTOCOL_MIN, "preedit": "both"});
    let connection = thread::spawn(move || serve_connection(server, sender, settings));
    (client, connection, worker)
}

/// 发一条、读一条；连接被断开时是 `None`。
fn exchange(client: &mut UnixStream, message: Value) -> Option<Value> {
    write_message(client, &message).unwrap();
    read_message::<_, Value>(client).unwrap()
}

/// 开会话并按 `version` 握手，返回 `LinuxHello` 的回包（被拒是 `None`）。
fn hello(client: &mut UnixStream, version: u32) -> Option<Value> {
    exchange(
        client,
        json!({"OpenSession": {"session": 1, "app": null, "protocol": PROTOCOL_VERSION}}),
    )
    .expect("开会话");
    exchange(
        client,
        json!({"LinuxHello": {"version": version, "session": 1, "generation": 1, "context": "version-test"}}),
    )
}

/// 握手之后报能力（普通输入框）、敲一个 `n`，返回按键的回包。
fn type_n(client: &mut UnixStream) -> Value {
    exchange(
        client,
        json!({"LinuxEvent": {"session": 1, "event": {"Capabilities": {"sensitive": false, "password": false, "disabled": false}}}}),
    )
    .expect("报能力");
    exchange(
        client,
        json!({"LinuxEvent": {"session": 1, "event": {"Key": {"release": false,
            "event": {"virtual_key": 78, "character": "n", "modifiers": {"ctrl": false, "shift": false, "alt": false, "win": false, "caps": false, "english_mode": false}}}}}}),
    )
    .expect("按键")
}

/// 还认的版本照常握手、回包回显前端报的那一版（Fcitx5 / Fcitx4 插件写死 3，要它原样回来）；太老太新都断开。
#[test]
fn hello_accepts_every_supported_version_and_echoes_it() {
    for version in [
        LINUX_UI_PROTOCOL_MIN - 1,
        LINUX_UI_PROTOCOL_MIN,
        LINUX_UI_PROTOCOL,
        LINUX_UI_PROTOCOL + 1,
    ] {
        let (mut client, connection, worker) = serve();
        let reply = hello(&mut client, version);
        if (LINUX_UI_PROTOCOL_MIN..=LINUX_UI_PROTOCOL).contains(&version) {
            let reply = reply.expect("握手成功");
            assert_eq!(reply["LinuxHello"]["version"], version);
            assert_eq!(reply["LinuxHello"]["session"], 1);
        } else {
            assert_eq!(reply, None, "版本 {version} 应当断开");
        }
        drop(client);
        connection.join().unwrap();
        worker.join().unwrap();
    }
}

/// 会话只能发它握手那一版里有的事件：v3 的会话发托盘切换当协议错误断开，v4 的会话照常切。
#[test]
fn newer_events_need_the_newer_handshake() {
    for version in [LINUX_UI_PROTOCOL_MIN, LINUX_UI_PROTOCOL] {
        let (mut client, connection, worker) = serve();
        hello(&mut client, version).expect("握手成功");
        let typed = type_n(&mut client);
        assert!(
            typed["KeyResult"]["mode"].is_object(),
            "老版本的会话回包上也附模式：{typed}"
        );
        let toggled = exchange(
            &mut client,
            json!({"LinuxEvent": {"session": 1, "event": {"Indicator": "ToggleEnglish"}}}),
        );
        if version < LINUX_UI_PROTOCOL {
            assert_eq!(toggled, None, "v{version} 的会话不该发托盘切换");
        } else {
            let toggled = toggled.expect("托盘切换的回包");
            assert_eq!(toggled["KeyResult"]["session"], 1);
            assert_eq!(toggled["KeyResult"]["english"], true);
        }
        drop(client);
        connection.join().unwrap();
        worker.join().unwrap();
    }
}

/// 右键菜单的回包不是 `KeyResult`，会话编号也得换回连接内的；v3 的会话不能问。
#[test]
fn menu_needs_v4_and_answers_with_the_local_session() {
    for version in [LINUX_UI_PROTOCOL_MIN, LINUX_UI_PROTOCOL] {
        let (mut client, connection, worker) = serve();
        hello(&mut client, version).expect("握手成功");
        let identity = type_n(&mut client)["KeyResult"]["identity"].clone();
        let menu = exchange(
            &mut client,
            json!({"LinuxEvent": {"session": 1, "event": {"Menu": {"identity": identity, "index": 0}}}}),
        );
        if version < LINUX_UI_PROTOCOL {
            assert_eq!(menu, None, "v{version} 的会话不该发右键菜单");
        } else {
            let menu = menu.expect("菜单回包");
            assert_eq!(menu["Menu"]["session"], 1, "会话编号换回连接内的");
            assert!(!menu["Menu"]["items"].as_array().unwrap().is_empty());
        }
        drop(client);
        connection.join().unwrap();
        worker.join().unwrap();
    }
}
