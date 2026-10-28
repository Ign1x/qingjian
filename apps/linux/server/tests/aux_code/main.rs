//! 辅码在 Linux Server 里的闭环：触发与逐键即筛、码段随帧、筛空与退格、标点先上屏，
//! 换会话不串、发给 Fcitx 插件的帧降级、码表与 `[aux_code]` 热加载，以及真的 Server 进程里的装配。
mod keys;
mod plugin;
mod reload;
#[cfg(target_os = "linux")]
#[path = "../support/server.rs"]
mod server;
mod sessions;
#[cfg(target_os = "linux")]
mod wired;

use std::sync::Arc;

use qingjian_core::Engine;
use qingjian_dictionary::{AuxCodeLookup, AuxCodeTable, Dictionary};
use qingjian_linux_server::{Router, RouterConfig};
use qingjian_platform::protocol::{
    ClientMessage, KeyEvent, KeyModifiers, PROTOCOL_VERSION, SessionId,
};
use serde_json::{Value, json};

pub const DICT: &str = "开发\tkai fa\t9000\n开发者\tkai fa zhe\t3000\n开放\tkai fang\t20000\n开饭\tkai fan\t800\n开\tkai\t20000\n";

pub const BACK: u32 = 0x08;
pub const ESCAPE: u32 = 0x1B;

/// `kaifa` 之后进辅码、`k` 筛到 开发 / 开发者，`kh` 只剩 开。
pub fn table() -> Arc<dyn AuxCodeLookup> {
    Arc::new(
        AuxCodeTable::from_pairs([
            ("开发".to_owned(), "kf".to_owned()),
            ("开发者".to_owned(), "kfz".to_owned()),
            ("开".to_owned(), "kh".to_owned()),
        ])
        .unwrap(),
    )
}

/// 装了码表、开着辅码的引擎；生产里这几样由 `main.rs` 按配置设。
pub fn engine(config: &RouterConfig) -> Engine {
    let mut engine = Engine::new(Dictionary::parse(DICT).unwrap());
    engine.set_aux_codes(vec![table()]);
    engine.set_aux_enabled(true);
    engine.set_aux_show(config.aux_code_show);
    engine.set_shift_letter_compose(config.shift_letter_compose);
    engine
}

/// 会话 1、2 按 `version` 握手（0 是不握手，Router 层的测试常这么开）并报过能力（普通输入框）。
pub fn open(engine: Engine, config: RouterConfig, version: u32) -> Router {
    let mut router = Router::new(engine, config);
    for id in [1, 2] {
        router.handle(ClientMessage::OpenSession {
            session: SessionId(id),
            app: None,
            protocol: PROTOCOL_VERSION,
        });
        let mut report = json!({"session": id, "identity": {"generation": 1, "context": format!("aux-{id}"), "revision": 0}});
        if version > 0 {
            report["version"] = json!(version);
        }
        router.handle_linux(json!({ "DisplayReporting": report }));
        router.handle_linux(json!({"LinuxEvent": {"session": id, "event": {"Capabilities": {"sensitive": false, "password": false, "disabled": false}}}}));
    }
    router
}

/// 缺省配置、握手 v4（Rust 前端）的 Router。
pub fn router() -> Router {
    let config = RouterConfig::default();
    open(engine(&config), config, 4)
}

/// 按下一个可打印键：键码按 Windows 的虚拟键码给（插件就是这么报的）。
pub fn key(router: &mut Router, id: u64, c: char) -> Value {
    let code = match c {
        'a'..='z' => c.to_ascii_uppercase() as u32,
        ';' => 0xBA,
        ',' => 0xBC,
        '.' => 0xBE,
        '/' => 0xBF,
        _ => c as u32,
    };
    let modifiers = KeyModifiers {
        shift: c.is_ascii_uppercase(),
        ..KeyModifiers::default()
    };
    press(router, id, KeyEvent::new(code, Some(c), modifiers))
}

/// 按下一个功能键（退格、Esc…）。
pub fn function(router: &mut Router, id: u64, code: u32) -> Value {
    press(
        router,
        id,
        KeyEvent::new(code, None, KeyModifiers::default()),
    )
}

fn press(router: &mut Router, id: u64, event: KeyEvent) -> Value {
    router
        .handle_linux(json!({"LinuxEvent": {"session": id, "event": {"Key": {"event": event, "release": false}}}}))
        .unwrap()["KeyResult"]
        .clone()
}

pub fn compose(router: &mut Router, id: u64, text: &str) -> Value {
    text.chars().fold(Value::Null, |_, c| key(router, id, c))
}

/// 拼音行整行。
pub fn preedit(result: &Value) -> String {
    segments(result).map(|(text, _)| text).collect()
}

/// 码段（`AuxCode` 段）的文字；没有就是 `None`。
pub fn code_segment(result: &Value) -> Option<String> {
    segments(result)
        .find(|(_, kind)| kind == "AuxCode")
        .map(|(text, _)| text)
}

fn segments(result: &Value) -> impl Iterator<Item = (String, String)> + '_ {
    result["frame"]["preedit"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|segment| {
            (
                segment["text"].as_str().unwrap_or_default().to_owned(),
                segment["kind"].as_str().unwrap_or_default().to_owned(),
            )
        })
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
