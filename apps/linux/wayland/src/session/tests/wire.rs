//! 走真 Unix socket 的假 Server，给 [`Link`](super::super::Link) 的测试用。
//!
//! 只照协议的形状答话（哪条有回包、回什么壳），不复刻引擎；也像真 Server（`apps/linux/server/src/ipc/connection.rs`）
//! 那样对不合规矩的消息直接断开：上下文为空、超过 64 字节或同一条连接上重复，报能力之前发别的事件，轮询没开的会话。
//! 记下每条连接收到的消息；能挂断当前连接，演 Server 重启。

use std::collections::HashMap;
use std::net::Shutdown;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use qingjian_platform::protocol::{Frame, KeyOutcome, read_message, write_message};
use serde_json::{Value, json};

/// 收到的消息：第几条连接（从 0 数）与消息本身。
type Received = Arc<Mutex<Vec<(usize, Value)>>>;

pub struct Wire {
    pub path: PathBuf,

    received: Received,

    /// 当前连接的另一个句柄，挂断用。
    live: Arc<Mutex<Option<UnixStream>>>,

    stop: Arc<AtomicBool>,

    thread: Option<JoinHandle<()>>,
}

impl Wire {
    pub fn start() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "qingjian-wayland-wire-{}-{}.sock",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path).expect("绑定假 Server 的 socket");
        let received = Received::default();
        let live = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (received, live, stop) = (received.clone(), live.clone(), stop.clone());
            std::thread::spawn(move || {
                for (index, stream) in listener.incoming().enumerate() {
                    if stop.load(Ordering::Relaxed) {
                        return;
                    }
                    let Ok(stream) = stream else {
                        continue;
                    };
                    *live.lock().expect("连接句柄") = stream.try_clone().ok();
                    serve(index, stream, &received);
                }
            })
        };
        Self {
            path,
            received,
            live,
            stop,
            thread: Some(thread),
        }
    }

    /// 到现在为止收到的消息。没有回包的（关会话、展示回执）要等后面一次有回包的往返完了才一定记上。
    pub fn received(&self) -> Vec<(usize, Value)> {
        self.received.lock().expect("收到的消息").clone()
    }

    /// 挂断当前连接（Server 重启了）：客户端下一次往返失败。
    pub fn hang_up(&self) {
        if let Some(stream) = self.live.lock().expect("连接句柄").take() {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }
}

impl Drop for Wire {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.hang_up();
        // 叫醒等在 accept 上的线程，它看到 stop 就收工
        let _ = UnixStream::connect(&self.path);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        let _ = std::fs::remove_file(&self.path);
    }
}

/// 服务一条连接。会话按编号记 `(上下文, 报过能力没有)`。
fn serve(index: usize, mut stream: UnixStream, received: &Mutex<Vec<(usize, Value)>>) {
    let mut sessions: HashMap<u64, (String, bool)> = HashMap::new();
    let mut revision = 0;
    while let Ok(Some(message)) = read_message::<_, Value>(&mut stream) {
        received
            .lock()
            .expect("记下收到的消息")
            .push((index, message.clone()));
        let reply = if let Some(open) = message.get("OpenSession") {
            Some(
                json!({"Update": {"session": open["session"], "frame": Frame::default(), "linux_ui": {}}}),
            )
        } else if let Some(hello) = message.get("LinuxHello") {
            let context = hello["context"].as_str().unwrap_or_default().to_owned();
            let taken = sessions.values().any(|(other, _)| *other == context);
            if context.is_empty() || context.len() > 64 || taken {
                return;
            }
            let session = hello["session"].as_u64().unwrap_or_default();
            sessions.insert(session, (context, false));
            Some(json!({"LinuxHello": {"session": session, "page_size": 9}}))
        } else if let Some(event) = message.get("LinuxEvent") {
            let session = event["session"].as_u64().unwrap_or_default();
            match sessions.get_mut(&session) {
                // 关掉了的会话：真 Server 回 Ignored，不断开
                None => Some(json!({"Ignored": {"session": session}})),
                Some((context, reported)) => {
                    let capabilities = event["event"].get("Capabilities").is_some();
                    if !*reported && !capabilities {
                        return;
                    }
                    *reported = true;
                    revision += 1;
                    Some(json!({"KeyResult": {
                        "session": session,
                        "outcome": KeyOutcome::Passthrough,
                        "commit": Value::Null,
                        "frame": Frame::default(),
                        "identity": {"generation": 1, "context": context, "revision": revision},
                    }}))
                }
            }
        } else if let Some(poll) = message.get("Poll") {
            let session = poll["session"].as_u64().unwrap_or_default();
            let Some((context, _)) = sessions.get(&session) else {
                return;
            };
            Some(json!({"Update": {
                "session": session,
                "frame": Frame::default(),
                "identity": {"generation": 1, "context": context, "revision": revision},
            }}))
        } else {
            if let Some(close) = message.get("CloseSession") {
                sessions.remove(&close["session"].as_u64().unwrap_or_default());
            }
            // 关会话与展示回执没有回包
            None
        };
        if let Some(reply) = reply
            && write_message(&mut stream, &reply).is_err()
        {
            return;
        }
    }
}
