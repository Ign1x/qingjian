//! 握手转来的显示能力落到会话上。
use crate::dispatch::{Router, RouterConfig};
use qingjian_core::Engine;
use qingjian_dictionary::Dictionary;
use qingjian_platform::protocol::linux::Features;
use qingjian_platform::protocol::{ClientMessage, PROTOCOL_VERSION, SessionId};
use serde_json::json;

fn report(router: &mut Router, features: Option<serde_json::Value>) -> Features {
    let mut body =
        json!({"session": 1, "identity": {"generation": 1, "context": "c", "revision": 0}});
    if let Some(features) = features {
        body["features"] = features;
    }
    router.handle_linux(json!({ "DisplayReporting": body }));
    router.sessions[&SessionId(1)].features
}

#[test]
fn reported_features_land_on_the_session() {
    let mut router = Router::new(
        Engine::new(Dictionary::parse("你\tni\t100\n").unwrap()),
        RouterConfig::default(),
    );
    router.handle(ClientMessage::OpenSession {
        session: SessionId(1),
        app: None,
        protocol: PROTOCOL_VERSION,
    });
    assert_eq!(report(&mut router, None), Features::default(), "没报按全关");
    assert_eq!(
        report(&mut router, Some(json!({"grid": true, "key_hint": true}))),
        Features {
            grid: true,
            key_hint: true
        }
    );
}
