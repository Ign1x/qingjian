//! Linux 的实验模型装配、切换与档位；重排行为通过假模型走真实 Router。
mod behavior;
mod config;
mod prefers;

use self::prefers::Prefers;
use crate::dispatch::{Router, RouterConfig};
use qingjian_core::Engine;
use qingjian_dictionary::Dictionary;
use qingjian_platform::models::LoadedModel;
use qingjian_platform::protocol::{
    ClientMessage, KeyEvent, KeyModifiers, PROTOCOL_VERSION, SessionId,
};
use std::sync::Arc;

fn router() -> Router {
    let engine = Engine::new(
        Dictionary::parse("你他\tni ta\t100\n你她\tni ta\t90\n你它\tni ta\t80\n").unwrap(),
    );
    let mut router = Router::new(engine, RouterConfig::default());
    router.handle(ClientMessage::OpenSession {
        session: SessionId(1),
        app: None,
        protocol: PROTOCOL_VERSION,
    });
    router.handle(ClientMessage::Privacy {
        session: SessionId(1),
        private: false,
    });
    router.ensure_focus(SessionId(1));
    router
}

fn compose(router: &mut Router) {
    for c in "nita".chars() {
        router.handle(ClientMessage::Key {
            session: SessionId(1),
            event: KeyEvent::new(
                c.to_ascii_uppercase() as u32,
                Some(c),
                KeyModifiers::default(),
            ),
        });
    }
}

fn attach(router: &mut Router, model: &Arc<Prefers>) {
    router.attach_model(LoadedModel::Choice(model.clone()));
}
