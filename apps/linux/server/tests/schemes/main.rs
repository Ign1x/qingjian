//! 输入方案在 Linux Server 里的装配：随包五笔与混输、用户目录 `schemes/` 里的双拼与形码方案、
//! 回包 `mode.scheme` 报真正在用的方案名，改配置、导入五笔词库后的热加载，以及真的 Server 进程里找随包方案。
mod custom;
mod reload;
#[cfg(target_os = "linux")]
#[path = "../support/server.rs"]
mod server;
#[cfg(target_os = "linux")]
mod wired;
mod wubi;

use std::path::{Path, PathBuf};

use qingjian_core::Engine;
use qingjian_dictionary::Dictionary;
use qingjian_linux_server::{Router, RouterConfig, find_bundled_schemes};
use qingjian_platform::Config;
use qingjian_platform::protocol::{
    ClientMessage, KeyEvent, KeyModifiers, PROTOCOL_VERSION, SessionId,
};
use serde_json::{Value, json};

pub const DICT: &str = "你好\tni hao\t100\n你\tni\t80\n好\thao\t70\n开发\tkai fa\t9000\n";

/// 每个测试自己的用户数据目录。
pub fn user_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("qingjian-schemes-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 仓库里的随包根：`assets/schemes/wubi86` 就在它下面。
pub fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// 照 `main.rs` 装配：按 `source` 写配置，用户目录是 `dir`，随包方案用仓库里的；会话 1 握好手、报过能力。
pub fn started(dir: &Path, source: &str) -> (Router, Config) {
    let path = dir.join("config.toml");
    std::fs::write(&path, source).unwrap();
    let config = Config::load(&path).unwrap();
    let engine = Engine::new(Dictionary::parse(DICT).unwrap());
    let mut router = Router::new(engine, RouterConfig::from(&config));
    router.configure_schemes(
        &config,
        Some(dir.to_owned()),
        find_bundled_schemes(&repository()),
    );
    router.handle(ClientMessage::OpenSession {
        session: SessionId(1),
        app: None,
        protocol: PROTOCOL_VERSION,
    });
    router.handle_linux(json!({"DisplayReporting": {"session": 1, "identity": {"generation": 1, "context": "schemes", "revision": 0}, "version": 4}}));
    router.handle_linux(json!({"LinuxEvent": {"session": 1, "event": {"Capabilities": {"sensitive": false, "password": false, "disabled": false}}}}));
    (router, config)
}

/// 按下一个可打印键，键码按插件报的 Windows 虚拟键码给。
pub fn key(router: &mut Router, c: char) -> Value {
    let code = match c {
        'a'..='z' => c.to_ascii_uppercase() as u32,
        ',' => 0xBC,
        _ => c as u32,
    };
    let event = KeyEvent::new(code, Some(c), KeyModifiers::default());
    router
        .handle_linux(json!({"LinuxEvent": {"session": 1, "event": {"Key": {"event": event, "release": false}}}}))
        .unwrap()["KeyResult"]
        .clone()
}

pub fn compose(router: &mut Router, text: &str) -> Value {
    text.chars().fold(Value::Null, |_, c| key(router, c))
}

/// 这一页候选的文字。
pub fn texts(result: &Value) -> Vec<String> {
    result["frame"]["candidates"]["items"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|item| item["text"].as_str().unwrap_or_default().to_owned())
        .collect()
}

/// 回包上的方案名（状态条的写法）。
pub fn scheme(result: &Value) -> String {
    result["mode"]["scheme"].as_str().unwrap().to_owned()
}
