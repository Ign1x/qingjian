//! 假的青简 Server：同一套线上格式（4 字节小端长度 + JSON），在测试进程里自己的线程上听一个 Unix socket。
//! 只复刻协议的形状、不复刻引擎（与 `qingjian_platform::protocol::linux::client` 的测试同一个思路）：
//! 按下与候选窗上的点击按测试排好的 [`Answer`] 答，别的照真 Server 的壳回，收到的每条消息都记下来给测试核对。

mod answer;
mod conversation;

pub use answer::Answer;
pub use conversation::MENU;

use std::collections::VecDeque;
use std::io::ErrorKind;
use std::net::Shutdown;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use qingjian_platform::protocol::{KeyEvent, read_message, write_message};
use serde_json::Value;

use conversation::Conversation;

use super::Log;

pub struct Server {
    /// 收到的每条消息，按先后。
    pub received: Log<Value>,

    /// 排着的按下答复，按先后用掉。
    presses: Arc<Mutex<VecDeque<Answer>>>,

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
        let presses = Arc::new(Mutex::new(VecDeque::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let connection = Arc::new(Mutex::new(None));
        let thread = thread::spawn({
            let received = received.clone();
            let presses = presses.clone();
            let stop = stop.clone();
            let connection = connection.clone();
            move || {
                while !stop.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            stream.set_nonblocking(false).expect("连接改回阻塞");
                            *lock(&connection) = stream.try_clone().ok();
                            serve(stream, &received, &presses);
                            *lock(&connection) = None;
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
            presses,
            stop,
            connection,
            thread: Some(thread),
        }
    }

    /// 排一个按下（或候选窗上的点候选、翻页、点菜单项）的答复。没排到的与所有抬起都放行、帧不变，与真 Server 一样。
    pub fn answer(&self, answer: Answer) {
        lock(&self.presses).push_back(answer);
    }

    /// 收到的某一种 `LinuxEvent`（`Candidate`、`Page`、`Menu`…）的内容，按先后。
    pub fn events(&self, name: &str) -> Vec<Value> {
        self.received
            .entries()
            .iter()
            .filter_map(|message| message.get("LinuxEvent")?.get("event")?.get(name).cloned())
            .collect()
    }

    /// 收到的按键，按先后：（按键，是不是抬起）。
    pub fn keys(&self) -> Vec<(KeyEvent, bool)> {
        self.received
            .entries()
            .iter()
            .filter_map(|message| {
                let key = message.get("LinuxEvent")?.get("event")?.get("Key")?;
                let event = serde_json::from_value(key.get("event")?.clone()).ok()?;
                Some((event, key.get("release")?.as_bool()?))
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

/// 服务一条连接到对端关掉为止。
fn serve(mut stream: UnixStream, received: &Log<Value>, presses: &Mutex<VecDeque<Answer>>) {
    let mut conversation = Conversation::default();
    while let Ok(Some(message)) = read_message::<_, Value>(&mut stream) {
        received.push(message.clone());
        if let Some(reply) = conversation.reply(&message, presses)
            && write_message(&mut stream, &reply).is_err()
        {
            return;
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
