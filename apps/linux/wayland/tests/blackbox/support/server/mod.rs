//! 假的青简 Server：同一套线上格式（4 字节小端长度 + JSON），在测试进程里自己的线程上听一个 Unix socket。
//! 只复刻协议的形状、不复刻引擎（与 `qingjian_platform::protocol::linux::client` 的测试同一个思路）：
//! 按下与候选窗上的点击按测试排好的 [`Answer`] 答，别的照真 Server 的壳回，收到的每条消息都记下来给测试核对；
//! 鼠标请求须由测试逐种允许，没想到的消息记成违规、断开连接（见 `conversation.rs`），测试结束时一条都不能有。

mod answer;
mod conversation;
mod hook;
mod opened;
mod report;
mod script;

#[cfg(test)]
mod tests;

pub use answer::Answer;
pub use conversation::MENU;
pub use report::KeyReport;

use std::io::ErrorKind;
use std::net::Shutdown;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use qingjian_platform::protocol::linux::{Capabilities, LinuxEvent};
use qingjian_platform::protocol::{CodecError, KeyEvent, read_message, write_message};
use serde_json::Value;

use conversation::Conversation;
use hook::Hook;
use script::Script;

use super::Log;

pub struct Server {
    /// 收到的每条消息，按先后。
    pub received: Log<Value>,

    /// 违规：真 Server 会因此断开连接的，或这些测试里前端不该发的。测试结束时一条都不能有。
    pub violations: Log<String>,

    /// 连接的进出：连上记 `true`，断开记 `false`。测试结束时要一样多（被测进程退出时连接跟着关）。
    pub connections: Log<bool>,

    /// 排着的答复与译文。
    script: Arc<Mutex<Script>>,

    stop: Arc<AtomicBool>,

    /// 正在服务的那条连接（另一个句柄）：收工时关掉它，服务线程才不会一直卡在读上。
    connection: Arc<Mutex<Option<UnixStream>>>,

    thread: Option<JoinHandle<()>>,
}

impl Server {
    pub fn start(path: &Path) -> Self {
        let listener = UnixListener::bind(path).expect("假 Server 的 socket");
        listener.set_nonblocking(true).expect("socket 改成非阻塞");
        let received = Log::default();
        let violations = Log::default();
        let connections = Log::default();
        let script = Arc::new(Mutex::new(Script::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let connection = Arc::new(Mutex::new(None));
        let thread = thread::spawn({
            let logs = (received.clone(), violations.clone(), connections.clone());
            let script = script.clone();
            let stop = stop.clone();
            let connection = connection.clone();
            move || {
                let (received, violations, connections) = logs;
                while !stop.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            stream.set_nonblocking(false).expect("连接改回阻塞");
                            *lock(&connection) = stream.try_clone().ok();
                            connections.push(true);
                            serve(stream, &received, &violations, &script);
                            *lock(&connection) = None;
                            connections.push(false);
                        }
                        Err(error) if error.kind() == ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(5));
                        }
                        Err(error) => panic!("假 Server 收连接失败：{error}"),
                    }
                }
            }
        });
        Self {
            received,
            violations,
            connections,
            script,
            stop,
            connection,
            thread: Some(thread),
        }
    }

    /// 排一个按下（或候选窗上的点候选、翻页、点菜单项）的答复。没排到的与所有抬起都放行、帧不变，与真 Server 一样。
    pub fn answer(&self, answer: Answer) {
        lock(&self.script).presses.push_back(answer);
    }

    /// 只允许当前测试会发的鼠标或托盘事件；未列出的操作仍算违规。
    pub fn allow(&self, events: &[&'static str]) {
        assert!(events.iter().all(|event| matches!(
            *event,
            "Candidate" | "Page" | "Menu" | "MenuPick" | "Indicator"
        )));
        lock(&self.script).allowed_events.extend(events);
    }

    /// 选区翻译的译文到了：下一次轮询换上它（还在等译文时才换）。
    pub fn translated(&self, text: &str) {
        lock(&self.script).translation = Some(text.to_owned());
    }

    pub fn before_reply(&self, kind: &'static str, action: impl FnOnce() + Send + 'static) {
        lock(&self.script).before_reply = Some(Hook {
            kind,
            action: Box::new(action),
        });
    }

    /// 收到的某一种 `LinuxEvent`（`Candidate`、`Page`、`Menu`…）的内容，按先后。
    pub fn pointer_events(&self, name: &str) -> Vec<Value> {
        self.received
            .entries()
            .iter()
            .filter_map(|message| message.get("LinuxEvent")?.get("event")?.get(name).cloned())
            .collect()
    }

    /// 收到的按键，按先后：（按键，是不是抬起）。
    pub fn keys(&self) -> Vec<(KeyEvent, bool)> {
        self.key_reports()
            .into_iter()
            .map(|report| (report.event, report.release))
            .collect()
    }

    /// 收到的按键连同它带的前后文与「能读选区」，按先后。
    pub fn key_reports(&self) -> Vec<KeyReport> {
        self.events()
            .into_iter()
            .filter_map(|event| match event {
                LinuxEvent::Key {
                    event,
                    release,
                    surrounding,
                    selection_supported,
                } => Some(KeyReport {
                    event,
                    release,
                    surrounding,
                    selection_supported,
                }),
                _ => None,
            })
            .collect()
    }

    /// 报来的输入框能力，按先后。
    pub fn capabilities(&self) -> Vec<Capabilities> {
        self.events()
            .into_iter()
            .filter_map(|event| match event {
                LinuxEvent::Capabilities(capabilities) => Some(capabilities),
                _ => None,
            })
            .collect()
    }

    /// 回给 Server 的选中文字，按先后。
    pub fn selections(&self) -> Vec<String> {
        self.events()
            .into_iter()
            .filter_map(|event| match event {
                LinuxEvent::Selection { text, .. } => Some(text),
                _ => None,
            })
            .collect()
    }

    /// 收到的 Linux 事件，按先后；解不开的跳过（那已经记成违规了）。
    fn events(&self) -> Vec<LinuxEvent> {
        self.received
            .entries()
            .iter()
            .filter_map(|message| {
                serde_json::from_value(message.get("LinuxEvent")?.get("event")?.clone()).ok()
            })
            .collect()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(connection) = lock(&self.connection).take() {
            let _ = connection.shutdown(Shutdown::Both);
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// 消息的种类：`LinuxEvent` 取里面事件的名字（`Capabilities`、`Focus`、`Key`…），别的取外层的名字（`OpenSession`、`Poll`…）。
pub fn kind(message: &Value) -> String {
    let Some((name, body)) = message.as_object().and_then(|object| object.iter().next()) else {
        return String::new();
    };
    if name != "LinuxEvent" {
        return name.clone();
    }
    match &body["event"] {
        Value::String(unit) => unit.clone(),
        Value::Object(event) => event.keys().next().cloned().unwrap_or_default(),
        _ => String::new(),
    }
}

/// 服务一条连接到对端关掉为止；收到违规的消息就记下来、断开，像真 Server 那样不再往下答。
fn serve(
    mut stream: UnixStream,
    received: &Log<Value>,
    violations: &Log<String>,
    script: &Mutex<Script>,
) {
    let mut conversation = Conversation::default();
    loop {
        let message = match read_message::<_, Value>(&mut stream) {
            Ok(Some(message)) => message,
            // 对端关了，或者收工时这边关了
            Ok(None) | Err(CodecError::Io(_)) => return,
            Err(error) => {
                violations.push(format!("读不出的消息：{error}"));
                return;
            }
        };
        received.push(message.clone());
        let (reply, before_reply) = {
            let mut script = lock(script);
            let reply = conversation.reply(&message, &mut script);
            let before_reply = if matches!(&reply, Ok(Some(_)))
                && script
                    .before_reply
                    .as_ref()
                    .is_some_and(|hook| hook.kind == kind(&message))
            {
                script.before_reply.take()
            } else {
                None
            };
            (reply, before_reply)
        };
        if let Some(before_reply) = before_reply {
            (before_reply.action)();
        }
        match reply {
            Ok(Some(reply)) => {
                if write_message(&mut stream, &reply).is_err() {
                    return;
                }
            }
            Ok(None) => {}
            Err(violation) => {
                violations.push(violation);
                return;
            }
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
