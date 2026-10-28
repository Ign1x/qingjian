//! 假 Server 的鼠标允许范围、展示身份与选区翻译：拒绝未允许的请求，过期点击不消耗脚本。

use qingjian_platform::protocol::PROTOCOL_VERSION;
use qingjian_platform::protocol::linux::key::to_key_event;
use qingjian_platform::protocol::linux::{
    Capabilities, DisplayIdentity, LINUX_UI_PROTOCOL, LinuxEvent, ModeCommand,
};
use serde_json::{Value, json};

use super::Answer;
use super::conversation::Conversation;
use super::script::Script;

fn open(
    conversation: &mut Conversation,
    script: &mut Script,
    session: u64,
    context: &str,
) -> DisplayIdentity {
    for message in [
        json!({"OpenSession": {"session": session, "protocol": PROTOCOL_VERSION}}),
        json!({"LinuxHello": {
            "session": session, "version": LINUX_UI_PROTOCOL, "generation": 1, "context": context,
        }}),
    ] {
        conversation.reply(&message, script).expect("合法握手");
    }
    let reply = conversation
        .reply(
            &event(session, LinuxEvent::Capabilities(Capabilities::default())),
            script,
        )
        .expect("报能力")
        .expect("能力回包");
    serde_json::from_value(reply["KeyResult"]["identity"].clone()).expect("展示身份")
}

fn event(session: u64, event: LinuxEvent) -> Value {
    json!({"LinuxEvent": {"session": session, "event": event}})
}

#[test]
fn pointer_permissions_do_not_allow_other_events() {
    let mut conversation = Conversation::default();
    let mut script = Script::default();
    let identity = open(&mut conversation, &mut script, 1, "wayland");
    for denied in [
        LinuxEvent::Candidate {
            identity: identity.clone(),
            index: 0,
        },
        LinuxEvent::Page {
            identity: identity.clone(),
            next: true,
        },
        LinuxEvent::Menu {
            identity: identity.clone(),
            index: 0,
        },
        LinuxEvent::MenuPick {
            identity: identity.clone(),
            index: 0,
            action: 1,
        },
    ] {
        assert!(conversation.reply(&event(1, denied), &mut script).is_err());
    }
    script.pointer_events.insert("Menu");
    for denied in [
        LinuxEvent::Candidate {
            identity: identity.clone(),
            index: 0,
        },
        LinuxEvent::Indicator(ModeCommand::ToggleEnglish),
        LinuxEvent::Selection {
            request: 99,
            text: "没要过".to_owned(),
        },
    ] {
        assert!(conversation.reply(&event(1, denied), &mut script).is_err());
    }
    let menu = event(1, LinuxEvent::Menu { identity, index: 0 });
    assert!(
        conversation
            .reply(&menu, &mut script)
            .expect("允许的菜单")
            .unwrap()
            .get("Menu")
            .is_some()
    );
}

#[test]
fn stale_pointer_identities_do_not_consume_answers() {
    let mut conversation = Conversation::default();
    let mut script = Script::default();
    let mut identity = open(&mut conversation, &mut script, 1, "first");
    let second = open(&mut conversation, &mut script, 2, "second");
    identity.revision = second.revision;
    script.pointer_events.extend(["Candidate", "Menu"]);
    script.presses.push_back(Answer::commit("正确候选"));
    for stale in [
        DisplayIdentity {
            generation: 2,
            ..identity.clone()
        },
        second,
        DisplayIdentity {
            revision: identity.revision - 1,
            ..identity.clone()
        },
    ] {
        let reply = conversation
            .reply(
                &event(
                    1,
                    LinuxEvent::Candidate {
                        identity: stale,
                        index: 0,
                    },
                ),
                &mut script,
            )
            .expect("过期只回 Ignored");
        assert_eq!(reply, Some(json!({"Ignored": {"session": 1}})));
        assert_eq!(script.presses.len(), 1, "旧点击不能用掉下一次答复");
    }
    let menu = event(
        1,
        LinuxEvent::Menu {
            identity: identity.clone(),
            index: 0,
        },
    );
    conversation.reply(&menu, &mut script).expect("问菜单");
    let reply = conversation
        .reply(
            &event(1, LinuxEvent::Candidate { identity, index: 0 }),
            &mut script,
        )
        .expect("菜单不推进身份")
        .expect("点选回包");
    assert_eq!(reply["KeyResult"]["commit"], "正确候选");
    assert_eq!(reply["KeyResult"]["identity"]["context"], "first");
    assert!(script.presses.is_empty());
}

#[test]
fn a_translation_can_be_committed_by_a_pointer_answer() {
    let mut conversation = Conversation::default();
    let mut script = Script::default();
    open(&mut conversation, &mut script, 1, "wayland");
    script.pointer_events.insert("Candidate");
    script
        .presses
        .extend([Answer::select(7), Answer::commit("apple")]);
    let key = LinuxEvent::Key {
        event: to_key_event('T' as u32, 0b1001),
        release: false,
        surrounding: None,
        selection_supported: true,
    };
    let reply = conversation
        .reply(&event(1, key), &mut script)
        .expect("要选区");
    assert_eq!(
        reply,
        Some(json!({"RequestSelection": {"session": 1, "request": 7}}))
    );
    conversation
        .reply(
            &event(
                1,
                LinuxEvent::Selection {
                    request: 7,
                    text: "苹果".to_owned(),
                },
            ),
            &mut script,
        )
        .expect("回选区");
    script.translation = Some("apple".to_owned());
    let reply = conversation
        .reply(&json!({"Poll": {"session": 1}}), &mut script)
        .expect("译文到了")
        .expect("轮询回包");
    assert_eq!(reply["Update"]["frame"]["translation_pending"], false);
    let identity = serde_json::from_value(reply["Update"]["identity"].clone()).expect("译文身份");
    let reply = conversation
        .reply(
            &event(1, LinuxEvent::Candidate { identity, index: 0 }),
            &mut script,
        )
        .expect("鼠标选译文")
        .expect("点选回包");
    assert_eq!(reply["KeyResult"]["commit"], "apple");
    assert_eq!(reply["KeyResult"]["frame"]["translation_pending"], false);
}
