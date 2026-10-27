//! 握手、按键、轮询与断线：会话这一层的往返。

use serde_json::json;

use super::fake_server;
use crate::protocol::linux::client::{ClientError, Connection};
use crate::protocol::linux::key::to_key_event;
use crate::protocol::linux::{Capabilities, LINUX_UI_PROTOCOL};
use crate::protocol::{KeyOutcome, PROTOCOL_VERSION};

/// 整条链路：开会话 → 报能力 → 按键 → 关会话，顺序与字段都要对得上 Server 的要求。
#[test]
fn handshake_then_key_round_trips() {
    let (path, received, server) = fake_server(None);
    let mut connection = Connection::connect_at(&path).expect("连上假 Server");

    let session = connection.open_session("/ibus/context/1").expect("开会话");
    let capabilities = connection
        .capabilities(session, Capabilities::default())
        .expect("报能力");
    assert_eq!(capabilities.outcome, KeyOutcome::Consumed);

    let key = to_key_event('n' as u32, 0);
    let reply = connection
        .key(session, key, false, None, false)
        .expect("报按键");
    assert_eq!(reply.outcome, KeyOutcome::Consumed);
    assert!(reply.commit.is_none());

    connection.close_session(session).expect("关会话");
    drop(connection);
    server.join().expect("假 Server 收工");
    let _ = std::fs::remove_file(&path);

    let seen = received.lock().expect("取回收到的消息");
    assert_eq!(seen.len(), 5, "开会话两条 + 能力 + 按键 + 关会话");

    // OpenSession 带的是 Windows 那套协议版本
    assert_eq!(seen[0]["OpenSession"]["protocol"], json!(PROTOCOL_VERSION));

    // LinuxHello：版本、代次、上下文三样 Server 都会查，对不上直接断开
    let hello = &seen[1]["LinuxHello"];
    assert_eq!(hello["version"], json!(LINUX_UI_PROTOCOL));
    assert!(
        hello["generation"].as_u64().expect("代次") > 0,
        "代次要大于 0"
    );
    assert_eq!(hello["context"], json!("/ibus/context/1"));

    // 能力必须排在按键前面：Server 在收到它之前不收按键
    assert!(seen[2]["LinuxEvent"]["event"]["Capabilities"].is_object());
    let key = &seen[3]["LinuxEvent"]["event"]["Key"];
    assert_eq!(key["event"]["virtual_key"], json!('n' as u32));
    assert_eq!(key["selection_supported"], json!(false));
}

/// 连不上就是连不上，不要在这里重试或者吞掉错误：上层按这个决定要不要放行按键。
#[test]
fn connecting_to_a_dead_socket_fails() {
    let path = std::env::temp_dir().join("qingjian-client-test-not-there.sock");
    let _ = std::fs::remove_file(&path);
    assert!(Connection::connect_at(&path).is_err());
}

/// 会话编号在一条连接里递增，Server 靠它区分同一个前端的多个输入上下文。
#[test]
fn sessions_are_numbered_per_connection() {
    let (path, _received, server) = fake_server(None);
    let mut connection = Connection::connect_at(&path).expect("连上假 Server");
    let first = connection.open_session("/ibus/context/1").expect("第一个");
    let second = connection.open_session("/ibus/context/2").expect("第二个");
    assert_ne!(first, second);
    drop(connection);
    server.join().expect("假 Server 收工");
    let _ = std::fs::remove_file(&path);
}

/// 假 Server 关掉之后再发就该报错，而不是卡在读上。
#[test]
fn closed_server_is_reported() {
    // 只服务开会话那两条，之后 Server 就没了
    let (path, _received, server) = fake_server(Some(2));
    let mut connection = Connection::connect_at(&path).expect("连上假 Server");
    let session = connection.open_session("/ibus/context/1").expect("开会话");
    server.join().expect("假 Server 收工");
    let _ = std::fs::remove_file(&path);

    let key = to_key_event('n' as u32, 0);
    let error = connection
        .key(session, key, false, None, false)
        .expect_err("对端没了要报错");
    assert!(
        matches!(error, ClientError::Closed | ClientError::Codec(_)),
        "应该是连接断了这一类，实际是 {error}"
    );
}

/// 组句期间的轮询：发的是 `Poll`，回包（`Update`）与按键的回包（`KeyResult`）里展示版本号都要读出来，
/// IBus 前端靠它判断本地整句模型的重排结果到了没有（版本号变新才重画）。
#[test]
fn poll_reads_display_revision() {
    let (path, received, server) = fake_server(None);
    let mut connection = Connection::connect_at(&path).expect("连上假 Server");
    let session = connection.open_session("/ibus/context/1").expect("开会话");
    connection
        .capabilities(session, Capabilities::default())
        .expect("报能力");

    let typed = connection
        .key(session, to_key_event('n' as u32, 0), false, None, false)
        .expect("按键");
    assert_eq!(typed.revision(), Some(7));
    let polled = connection.poll(session).expect("轮询");
    assert_eq!(polled.revision(), Some(8));

    connection.close_session(session).expect("关会话");
    drop(connection);
    server.join().expect("假 Server 正常收工");
    let seen = received.lock().expect("取回收到的消息");
    assert!(
        seen.contains(&json!({"Poll": {"session": session}})),
        "轮询要发 Poll：{seen:?}"
    );
}
