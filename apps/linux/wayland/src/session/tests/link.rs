//! [`Link`] 对着走真 socket 的假 Server：一个应用一个会话、开着的太多就关最久没用的、重连后只补开当前应用的会话。

use qingjian_platform::protocol::linux::Capabilities;
use qingjian_platform::protocol::linux::key::to_key_event;
use serde_json::Value;

use super::super::link::context;
use super::super::{Link, Server};
use super::wire::Wire;

fn link(wire: &Wire) -> Link {
    Link::new(wire.path.clone()).without_backoff()
}

/// 会话层激活一个输入框时对 Link 做的：选应用、报能力、报焦点。
fn enter(link: &mut Link, app: Option<&str>, capabilities: Capabilities) {
    link.select(app);
    link.capabilities(capabilities).expect("报能力");
    link.focus(true).expect("报焦点");
}

/// 输入框失活：失焦，再 Deactivate。
fn leave(link: &mut Link) {
    link.focus(false).expect("失焦");
    link.deactivate().expect("失活");
}

fn press(link: &mut Link) {
    link.key(to_key_event('n' as u32, 0), false, None, false)
        .expect("按键");
}

fn sensitive() -> Capabilities {
    Capabilities {
        sensitive: true,
        ..Capabilities::default()
    }
}

/// 收到的消息一条一行：`<第几条连接> <消息> <会话编号>`。开会话带上应用（`-` 是不知道哪个应用），
/// 焦点带上进出，能力只列开着的几样。上下文另由 [`contexts`] 核对。
fn log(wire: &Wire) -> Vec<String> {
    wire.received()
        .iter()
        .map(|(connection, message)| describe(*connection, message))
        .collect()
}

fn describe(connection: usize, message: &Value) -> String {
    let (name, body) = message
        .as_object()
        .and_then(|object| object.iter().next())
        .expect("一条消息一个键");
    let session = &body["session"];
    if name == "OpenSession" {
        let app = body["app"].as_str().unwrap_or("-");
        return format!("{connection} OpenSession {session} {app}");
    }
    if name != "LinuxEvent" {
        return format!("{connection} {name} {session}");
    }
    let event = &body["event"];
    let Some((kind, fields)) = event.as_object().and_then(|object| object.iter().next()) else {
        // 没有字段的事件（Reset）是一个字符串
        return format!("{connection} {} {session}", event.as_str().unwrap_or("?"));
    };
    let detail = match kind.as_str() {
        "Focus" => format!(" {}", fields["focused"]),
        "Capabilities" => ["sensitive", "password"]
            .into_iter()
            .filter(|flag| fields[*flag] == true)
            .map(|flag| format!(" {flag}"))
            .collect(),
        _ => String::new(),
    };
    format!("{connection} {kind} {session}{detail}")
}

/// 握手报的上下文，按先后。
fn contexts(wire: &Wire) -> Vec<String> {
    wire.received()
        .iter()
        .filter_map(|(_, message)| message.get("LinuxHello"))
        .map(|hello| hello["context"].as_str().unwrap_or_default().to_owned())
        .collect()
}

/// 每个应用一个会话，第一次用到时才开：`OpenSession` 带 app_id，上下文是它的哈希。
/// 焦点、按键、轮询与回执都发给当前应用的会话；回到用过的应用不重开，能力没变也不重报。
#[test]
fn each_app_has_its_own_session() {
    let wire = Wire::start();
    let mut link = link(&wire);
    enter(&mut link, Some("firefox"), Capabilities::default());
    press(&mut link);
    leave(&mut link);
    enter(&mut link, Some("foot"), sensitive());
    press(&mut link);
    leave(&mut link);
    enter(&mut link, Some("firefox"), Capabilities::default());
    let polled = link.poll().expect("轮询");
    let identity = polled.identity.expect("回包带着展示身份");
    link.acknowledge(identity, vec![(0, 0)]).expect("回执");
    press(&mut link);

    assert_eq!(
        log(&wire),
        [
            "0 OpenSession 1 firefox",
            "0 LinuxHello 1",
            "0 Capabilities 1",
            "0 Focus 1 true",
            "0 Key 1",
            "0 Focus 1 false",
            "0 Deactivate 1",
            "0 OpenSession 2 foot",
            "0 LinuxHello 2",
            "0 Capabilities 2 sensitive",
            "0 Focus 2 true",
            "0 Key 2",
            "0 Focus 2 false",
            "0 Deactivate 2",
            "0 Focus 1 true",
            "0 Poll 1",
            "0 DisplayAcknowledged 1",
            "0 Key 1",
        ]
    );
    assert_eq!(
        contexts(&wire),
        [context(Some("firefox")), context(Some("foot"))]
    );
}

/// 开着的会话到了 16 个，再开新的就关掉最久没用的；被关掉的应用再来时重开一个（它的中 / 英状态回到缺省）。
#[test]
fn the_least_recently_used_session_is_closed() {
    let wire = Wire::start();
    let mut link = link(&wire);
    for index in 0..16 {
        enter(
            &mut link,
            Some(&format!("app{index}")),
            Capabilities::default(),
        );
        leave(&mut link);
    }
    // app0 又用了一次，最久没用的成了 app1
    enter(&mut link, Some("app0"), Capabilities::default());
    leave(&mut link);
    enter(&mut link, Some("app16"), Capabilities::default());
    leave(&mut link);
    enter(&mut link, Some("app1"), Capabilities::default());

    let log = log(&wire);
    let closed: Vec<&str> = log
        .iter()
        .filter(|line| line.contains("CloseSession"))
        .map(String::as_str)
        .collect();
    assert_eq!(
        closed,
        ["0 CloseSession 2", "0 CloseSession 3"],
        "开 app16 时关 app1，重开 app1 时再关 app2"
    );
    assert!(
        log.windows(2)
            .any(|pair| pair == ["0 CloseSession 2", "0 OpenSession 17 app16"]),
        "{log:#?}"
    );
    assert!(
        log.windows(2)
            .any(|pair| pair == ["0 CloseSession 3", "0 OpenSession 18 app1"]),
        "{log:#?}"
    );
}

/// Server 重启：下一次往返失败、连接扔掉；再连上时只补开当前应用的会话，先报能力、再补报焦点，
/// 别的应用等用到再开，上下文与重启前一样。
#[test]
fn reconnecting_reopens_only_the_current_session() {
    let wire = Wire::start();
    let mut link = link(&wire);
    enter(&mut link, Some("firefox"), Capabilities::default());
    leave(&mut link);
    enter(&mut link, Some("foot"), sensitive());
    wire.hang_up();
    assert!(
        link.key(to_key_event('n' as u32, 0), false, None, false)
            .is_err()
    );
    assert!(!link.connected());

    // 会话层到点重连走的就是报焦点这一句：补报的一次，加上这次请求本身
    link.focus(true).expect("重连");
    leave(&mut link);
    enter(&mut link, Some("firefox"), Capabilities::default());

    let again: Vec<String> = log(&wire)
        .into_iter()
        .filter(|line| line.starts_with("1 "))
        .collect();
    assert_eq!(
        again,
        [
            "1 OpenSession 1 foot",
            "1 LinuxHello 1",
            "1 Capabilities 1 sensitive",
            "1 Focus 1 true",
            "1 Focus 1 true",
            "1 Focus 1 false",
            "1 Deactivate 1",
            "1 OpenSession 2 firefox",
            "1 LinuxHello 2",
            "1 Capabilities 2",
            "1 Focus 2 true",
        ]
    );
    let (firefox, foot) = (context(Some("firefox")), context(Some("foot")));
    assert_eq!(
        contexts(&wire),
        [firefox.clone(), foot.clone(), foot, firefox]
    );
}

/// 不知道是哪个应用（合成器不报活动窗口）：所有输入框共用一个会话，`app` 报空，上下文是原来的 `wayland`。
#[test]
fn without_an_app_one_session_is_shared() {
    let wire = Wire::start();
    let mut link = link(&wire);
    enter(&mut link, None, Capabilities::default());
    leave(&mut link);
    enter(&mut link, None, Capabilities::default());
    press(&mut link);
    assert_eq!(
        log(&wire),
        [
            "0 OpenSession 1 -",
            "0 LinuxHello 1",
            "0 Capabilities 1",
            "0 Focus 1 true",
            "0 Focus 1 false",
            "0 Deactivate 1",
            "0 Focus 1 true",
            "0 Key 1",
        ]
    );
    assert_eq!(contexts(&wire), ["wayland"]);
}

/// 密码框只记下能力，不为还没用过的应用开会话；开着的会话照报，Server 据此丢掉组句、进隐私模式。
#[test]
fn a_password_field_only_reports_capabilities() {
    let wire = Wire::start();
    let mut link = link(&wire);
    let password = Capabilities {
        password: true,
        ..Capabilities::default()
    };
    link.select(Some("bank"));
    link.capabilities(password).expect("记下能力");
    assert!(!link.connected(), "不为密码框连 Server");

    enter(&mut link, Some("firefox"), Capabilities::default());
    leave(&mut link);
    link.capabilities(password).expect("报能力");
    link.select(Some("bank"));
    link.capabilities(Capabilities::default())
        .expect("记下能力");
    link.focus(true).expect("报焦点");
    assert_eq!(
        log(&wire),
        [
            "0 OpenSession 1 firefox",
            "0 LinuxHello 1",
            "0 Capabilities 1",
            "0 Focus 1 true",
            "0 Focus 1 false",
            "0 Deactivate 1",
            "0 Capabilities 1 password",
            "0 OpenSession 2 bank",
            "0 LinuxHello 2",
            "0 Capabilities 2",
            "0 Focus 2 true",
        ]
    );
}

/// 选区翻译的两步都发给当前应用的会话：按键带上 `selection_supported`，回选区带着 Server 给的请求编号与选中的文字。
#[test]
fn the_selection_goes_to_the_current_session() {
    let wire = Wire::start();
    let mut link = link(&wire);
    enter(&mut link, Some("firefox"), Capabilities::default());
    link.key(to_key_event('T' as u32, 0b1001), false, None, true)
        .expect("按键");
    link.selection(7, "苹果".to_owned()).expect("回选区");
    let received = wire.received();
    let events: Vec<&Value> = received
        .iter()
        .filter_map(|(_, message)| message.get("LinuxEvent"))
        .filter(|event| event["session"] == 1)
        .map(|event| &event["event"])
        .collect();
    assert_eq!(events[events.len() - 2]["Key"]["selection_supported"], true);
    assert_eq!(
        events[events.len() - 1]["Selection"],
        serde_json::json!({"request": 7, "text": "苹果"})
    );
}
