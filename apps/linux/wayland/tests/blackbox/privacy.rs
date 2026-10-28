//! 隐私：密码框不开 Server 会话、不接管键盘，里面敲的键不到 Server；敏感框照常组句，但不带前后文、不报能读选区。

use qingjian_platform::protocol::linux::Capabilities;

use super::support::{Answer, Field, KEY_A, KEY_N, Request, Rig, Setup, kind};

/// 被测进程处理密码框激活时的日志（`src/session/focus.rs`）：处理完它不向合成器与 Server 发任何东西，只能看日志知道处理过了。
const PASSWORD: &str = "密码框：不报焦点";

/// 一上来就是密码框（还没用过的应用里）：不为它连 Server、不开会话，也不接管键盘，键盘一直归应用。
#[test]
fn a_first_password_field_is_left_to_the_app() {
    let Some(mut rig) = Rig::start(Setup::default()) else {
        return;
    };
    rig.compositor.activate_with(Field::password());
    rig.frontend.wait_log("处理密码框的激活", PASSWORD);
    // 收尾时被测进程冲完发出去的请求才断开，之后的记录就是全部
    rig.stop();
    assert!(
        !rig.compositor.requests.entries().contains(&Request::Grab),
        "密码框接管了键盘"
    );
    let received = rig.server.received.entries();
    assert!(received.is_empty(), "为密码框连了 Server：{received:#?}");
}

/// 同一个应用里换到密码框：先按失活报 Server，再报 `Capabilities{password}`，不报焦点；换的时候还按着的键等它抬起、
/// 照样转给应用，这期间敲进密码框的键原样转给应用、不报 Server；都抬起之后交还键盘，之后的键直接归应用。
#[test]
fn keys_typed_into_a_password_field_never_reach_the_server() {
    let Some(rig) = Rig::start(Setup::default()) else {
        return;
    };
    rig.focus();
    let mark = rig.compositor.requests.len();
    rig.compositor.press(KEY_A);
    rig.compositor
        .requests
        .wait(mark, "转给应用的按下", |request| {
            matches!(request, Request::Key { pressed: true, .. })
        });
    let reported = rig.server.received.len();
    rig.compositor.activate_with(Field::password());
    rig.server.received.wait(reported, "报密码框", |message| {
        message["LinuxEvent"]["event"]["Capabilities"]["password"] == true
    });

    let keys = rig.server.keys().len();
    let mark = rig.compositor.requests.len();
    rig.compositor.tap(KEY_N);
    rig.compositor.release(KEY_A);
    rig.compositor
        .requests
        .wait(mark, "交还键盘", |request| *request == Request::Release);
    // 交还键盘时被测进程已经处理完激活与这几个键，报给 Server 的就是全部
    let kinds: Vec<String> = rig
        .server
        .received
        .since(reported)
        .iter()
        .map(kind)
        .collect();
    assert_eq!(kinds, ["Focus", "Deactivate", "Capabilities"]);
    let forwarded: Vec<(u32, bool)> = rig
        .compositor
        .requests
        .since(mark)
        .iter()
        .filter_map(|request| match request {
            Request::Key { code, pressed, .. } => Some((*code, *pressed)),
            _ => None,
        })
        .collect();
    assert_eq!(forwarded, [(KEY_N, true), (KEY_N, false), (KEY_A, false)]);
    assert_eq!(rig.server.keys().len(), keys, "密码框里敲的键报给了 Server");
    assert!(!rig.compositor.grabbed(), "交还之后键盘直接归应用");
}

/// 密码框点了「显示密码」（内容类型改回普通）：当成换了一个输入框，报回普通能力与焦点、重新接管键盘，键又交给 Server。
#[test]
fn showing_the_password_brings_the_input_method_back() {
    let Some(rig) = Rig::start(Setup::default()) else {
        return;
    };
    rig.focus();
    let mark = rig.compositor.requests.len();
    rig.compositor.update(Field::password());
    rig.compositor
        .requests
        .wait(mark, "交还键盘", |request| *request == Request::Release);

    let mark = rig.compositor.requests.len();
    let reported = rig.server.received.len();
    rig.compositor.update(Field::normal());
    rig.compositor
        .requests
        .wait(mark, "重新接管键盘", |request| {
            *request == Request::Grab
        });
    let kinds: Vec<String> = rig
        .server
        .received
        .since(reported)
        .iter()
        .map(kind)
        .collect();
    assert_eq!(kinds, ["Capabilities", "Focus"]);
    assert_eq!(
        rig.server.capabilities().last(),
        Some(&Capabilities::default())
    );

    rig.server.answer(Answer::composing("n", &["你"]));
    let reported = rig.server.received.len();
    rig.compositor.tap(KEY_N);
    rig.server
        .received
        .wait(reported, "报给 Server 的按键", |message| {
            kind(message) == "Key"
        });
}

/// 敏感输入框（hint sensitive_data）：报 `Capabilities{sensitive}`，照常组句，但按键不带前后文，有选区也不报能读选区。
#[test]
fn a_sensitive_field_keeps_its_text_to_itself() {
    let Some(rig) = Rig::start(Setup::default()) else {
        return;
    };
    rig.focus_with(Field::text("我喜欢苹果。", 15, 9).sensitive());
    assert!(
        rig.server
            .capabilities()
            .last()
            .is_some_and(|capabilities| capabilities.sensitive),
        "{:?}",
        rig.server.capabilities()
    );
    rig.server.answer(Answer::composing("n", &["你"]));
    let mark = rig.compositor.requests.len();
    rig.compositor.tap(KEY_N);
    rig.compositor.requests.wait(mark, "行内拼音", |request| {
        matches!(request, Request::Commit(_))
    });
    // 抬起也处理完
    rig.compositor.sync();
    let reports = rig.server.key_reports();
    assert_eq!(reports.len(), 2, "{reports:#?}");
    assert!(
        reports
            .iter()
            .all(|report| report.surrounding.is_none() && !report.selection_supported),
        "{reports:#?}"
    );
}
