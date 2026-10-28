//! 键位提示过 socket：握手报了 `features.key_hint` 的连接，回包上带着键盘、共用客户端读得出来；没报的（C++ 插件、IBus）什么都没有。
#![cfg(target_os = "linux")]
#[path = "support/server.rs"]
mod server;

use qingjian_platform::protocol::linux::client::Connection;
use qingjian_platform::protocol::linux::key::to_key_event;
use qingjian_platform::protocol::linux::{Capabilities, Features, KeyHintContent};
use server::Server;

#[test]
fn key_hint_reaches_only_clients_that_can_draw_it() {
    let directory = std::env::temp_dir().join(format!("qingjian-key-hint-{}", std::process::id()));
    std::fs::create_dir_all(directory.join("config/qingjian")).unwrap();
    std::fs::write(
        directory.join("config/qingjian/config.toml"),
        "[general]\nscheme = \"xiaohe\"\nkey_hint = \"always\"\nkey_hint_opacity = 10\nkey_hint_size = 120\n",
    )
    .unwrap();
    let mut server = Server::start(directory.clone());
    drop(server.connect());
    let can_draw = Features {
        key_hint: true,
        ..Features::default()
    };
    for (features, attached) in [(Features::default(), false), (can_draw, true)] {
        let mut connection = Connection::connect_at(&directory.join("server.sock"))
            .unwrap()
            .with_features(features);
        let session = connection.open_session("key-hint-test", None).unwrap();
        connection
            .capabilities(session, Capabilities::default())
            .unwrap();
        connection.focus(session, true).unwrap();
        let reply = connection
            .key(session, to_key_event('k' as u32, 0), false, None, false)
            .unwrap();
        assert!(!reply.frame.preedit.is_empty());
        assert_eq!(reply.key_hint.is_some(), attached, "{features:?}");
        if let Some(hint) = reply.key_hint {
            assert!(
                matches!(&hint.content, KeyHintContent::Shuangpin(board) if board.pending == Some('k'))
            );
            assert_eq!(
                (hint.opacity, hint.size),
                (30, 120),
                "写出范围的不透明度夹到 30%"
            );
        }
        assert_eq!(
            connection.poll(session).unwrap().key_hint.is_some(),
            attached
        );
    }
    drop(server);
    std::fs::remove_dir_all(&directory).unwrap();
}
