//! 带调读音只进辅助行；可能被框架提交的 preedit 始终保留原串。

use std::time::Duration;

use futures_util::StreamExt;
use qingjian_platform::protocol::{Frame, PreeditKind, PreeditSegment};
use zbus::object_server::SignalEmitter;
use zbus::zvariant::Value;
use zbus::{MatchRule, MessageStream, connection::Builder};

use super::bus::Bus;
use crate::ibus::engine::present_frame;

fn text(value: Value<'_>) -> String {
    let Value::Structure(structure) = value else {
        panic!("应为 IBusText")
    };
    assert_eq!(
        <&str>::try_from(&structure.fields()[0]).unwrap(),
        "IBusText"
    );
    <&str>::try_from(&structure.fields()[2]).unwrap().to_owned()
}

#[tokio::test]
async fn tones_use_auxiliary_while_preedit_keeps_commit_text() {
    let bus = Bus::start();
    let sender = Builder::address(bus.address.as_str())
        .unwrap()
        .build()
        .await
        .unwrap();
    let receiver = Builder::address(bus.address.as_str())
        .unwrap()
        .build()
        .await
        .unwrap();
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .interface("org.freedesktop.IBus.Engine")
        .unwrap()
        .path("/qingjian/test")
        .unwrap()
        .build();
    let mut signals = MessageStream::for_match_rule(rule, &receiver, Some(32))
        .await
        .unwrap();
    let emitter = SignalEmitter::new(&sender, "/qingjian/test").unwrap();
    let segment = |text: &str| PreeditSegment {
        text: text.to_owned(),
        kind: PreeditKind::Typed,
    };
    let frame = Frame {
        preedit: vec![segment("ni'hao")],
        cursor: 6,
        window_preedit: Some(Box::new((vec![segment("nǐ'hǎo")], 6))),
        ..Frame::default()
    };
    for toned in [true, false] {
        let frame = Frame {
            window_preedit: toned.then(|| frame.window_preedit.clone()).flatten(),
            ..frame.clone()
        };
        present_frame(&emitter, &frame).await.unwrap();
        let mut preedit = false;
        let mut auxiliary = false;
        tokio::time::timeout(Duration::from_secs(3), async {
            while !(preedit && auxiliary) {
                let message = signals.next().await.unwrap().unwrap();
                let body = message.body();
                match message.header().member().unwrap().as_str() {
                    "UpdatePreeditText" => {
                        let (value, cursor, visible, mode): (Value<'_>, u32, bool, u32) =
                            body.deserialize().unwrap();
                        assert_eq!(text(value), "ni'hao");
                        assert_eq!((cursor, visible, mode), (6, true, 1));
                        preedit = true;
                    }
                    "UpdateAuxiliaryText" => {
                        let (value, visible): (Value<'_>, bool) = body.deserialize().unwrap();
                        assert_eq!(text(value), if toned { "nǐ'hǎo" } else { "" });
                        assert_eq!(visible, toned);
                        auxiliary = true;
                    }
                    _ => {}
                }
            }
        })
        .await
        .expect("应收到 preedit 与辅助行信号");
    }
}
