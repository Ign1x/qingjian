//! 被测进程：真的 `qingjian-linux-wayland` 二进制，环境全指向沙箱，`--stderr` 把日志打到管道里收下来。

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStderr, Command, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::{Exit, Log, PATIENCE, Sandbox};

/// 就绪的那行日志：装好 SIGTERM / SIGINT 的处理、进主循环之前打（`src/wayland/mod.rs` 的 `run`）。
const READY: &str = "输入法就绪";

/// 渲染器建好了的那行日志（`src/view/mod.rs` 的 `Painter::new`）；没有说明这台机器上没有界面字体，候选窗画不出来。
const PAINTER: &str = "候选窗使用青简渲染器";

/// 缺运行库时二进制打的错，与跳过的说明。CI 里 ubuntu:22.04 那个裸容器没有 libxkbcommon；
/// 整个 workspace 一起编时 winit 把 wayland-backend 统一成 dlopen libwayland 的那一种，缺 libwayland-client 也起不来。
const MISSING: [(&str, &str); 2] = [
    (
        "libxkbcommon.so.0 could not be loaded",
        "加载不了 libxkbcommon（NixOS 上用 LD_LIBRARY_PATH 指过去）",
    ),
    (
        "The wayland library could not be loaded",
        "加载不了 libwayland-client（与整个 workspace 一起编时它是 dlopen 的）",
    ),
];

pub struct Frontend {
    child: Child,

    /// stderr 的每一行。
    lines: Log<String>,

    /// 读 stderr 的线程；进程退出、管道关上它就结束。
    reader: Option<JoinHandle<()>>,

    /// 测试已经收过它的退出状态（[`Self::exit`]，或起不来时 [`Self::ready`]），收尾时不再查。
    checked: bool,
}

impl Frontend {
    /// 起被测进程：Wayland 连 `display`（相对沙箱这个 XDG_RUNTIME_DIR），Server 连 `server`，
    /// 配置、主题、日志与 HOME 都在沙箱里，碰不到真会话与用户的文件。
    pub fn spawn(sandbox: &Sandbox, display: &str, server: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_qingjian-linux-wayland"))
            .arg("--stderr")
            .env("WAYLAND_DISPLAY", display)
            .env("XDG_RUNTIME_DIR", sandbox.root())
            .env("QINGJIAN_SOCKET", server)
            .env("HOME", sandbox.path("home"))
            .env("XDG_CONFIG_HOME", sandbox.path("config"))
            .env("XDG_DATA_HOME", sandbox.path("data"))
            .env("XDG_STATE_HOME", sandbox.path("state"))
            .env("XDG_CACHE_HOME", sandbox.path("cache"))
            .env("RUST_LOG", "info,qingjian_linux_wayland=debug")
            .env("NO_COLOR", "1")
            // 有 WAYLAND_SOCKET 时客户端不看 WAYLAND_DISPLAY，会连到继承下来的真会话上
            .env_remove("WAYLAND_SOCKET")
            .env_remove("WAYLAND_DEBUG")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("起被测进程");
        let stderr = child.stderr.take().expect("stderr 的管道");
        let lines = Log::default();
        let reader = thread::spawn({
            let lines = lines.clone();
            move || read_lines(stderr, &lines)
        });
        Self {
            child,
            lines,
            reader: Some(reader),
            checked: false,
        }
    }

    /// 等它就绪。缺运行库起不来时打印跳过说明、返回 `false`；别的原因退出了就 panic。
    pub fn ready(&mut self) -> bool {
        let started = Instant::now();
        loop {
            if self
                .lines
                .wait_for(0, Duration::from_millis(50), |line| line.contains(READY))
                .is_some()
            {
                return true;
            }
            if let Some(status) = self.child.try_wait().expect("查被测进程的状态") {
                let stderr = self.drain();
                if let Some(reason) = missing_library(&stderr) {
                    self.checked = true;
                    skip(reason);
                    return false;
                }
                panic!("被测进程没就绪就退出了（{status}）：\n{stderr}");
            }
            assert!(
                started.elapsed() < PATIENCE,
                "被测进程 {PATIENCE:?} 内没有就绪：\n{}",
                self.stderr()
            );
        }
    }

    /// 等它退出。缺运行库起不来时打印跳过说明、返回 `None`。
    pub fn exit(&mut self) -> Option<Exit> {
        let started = Instant::now();
        let status = loop {
            if let Some(status) = self.child.try_wait().expect("查被测进程的状态") {
                break status;
            }
            assert!(
                started.elapsed() < PATIENCE,
                "被测进程 {PATIENCE:?} 内没有退出：\n{}",
                self.stderr()
            );
            thread::sleep(Duration::from_millis(10));
        };
        let stderr = self.drain();
        self.checked = true;
        if let Some(reason) = missing_library(&stderr) {
            skip(reason);
            return None;
        }
        Some(Exit {
            code: status.code(),
            stderr,
        })
    }

    /// 收尾：测试没收过退出状态的话发 SIGTERM、等它退出，交出退出状态；收过的是 `None`。
    /// 测试中途它自己退了（崩了、被合成器断开）是错。
    pub fn finish(&mut self) -> Option<Exit> {
        if self.checked {
            return None;
        }
        if let Some(status) = self.child.try_wait().expect("查被测进程的状态") {
            panic!("被测进程在测试中途退出了（{status}）：\n{}", self.drain());
        }
        self.terminate();
        self.exit()
    }

    /// 发 SIGTERM（systemd 停服务、会话退出时就是它）。
    pub fn terminate(&self) {
        let pid = libc::pid_t::try_from(self.child.id()).expect("pid");
        // SAFETY: 只是给自己起的子进程发信号
        let sent = unsafe { libc::kill(pid, libc::SIGTERM) };
        assert_eq!(sent, 0, "发 SIGTERM 失败");
    }

    /// 渲染器建起来了（机器上有界面字体）：没有的话候选窗画不出来。
    pub fn painter(&self) -> bool {
        self.lines
            .entries()
            .iter()
            .any(|line| line.contains(PAINTER))
    }

    /// 等日志里出现含 `needle` 的一行：被测进程处理完某件事、又不向合成器或 Server 发任何东西时，靠它知道处理过了。
    pub fn wait_log(&self, what: &str, needle: &str) {
        self.lines.wait(0, what, |line| line.contains(needle));
    }

    /// 到现在为止的日志。
    pub fn log(&self) -> Vec<String> {
        self.lines.entries()
    }

    fn stderr(&self) -> String {
        self.lines.entries().join("\n")
    }

    /// 进程已经退出：等读线程把管道里剩下的读完，取回全部 stderr。
    fn drain(&mut self) -> String {
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        self.stderr()
    }
}

impl Drop for Frontend {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
        if thread::panicking() {
            eprintln!("被测进程的日志：\n{}", self.drain());
        }
    }
}

fn read_lines(stderr: ChildStderr, lines: &Log<String>) {
    for line in BufReader::new(stderr).split(b'\n') {
        let Ok(line) = line else {
            return;
        };
        lines.push(String::from_utf8_lossy(&line).into_owned());
    }
}

fn missing_library(stderr: &str) -> Option<&'static str> {
    MISSING
        .iter()
        .find(|(error, _)| stderr.contains(error))
        .map(|(_, reason)| *reason)
}

/// 跳过的说明直接写进 stderr、不经测试框架的输出捕获：通过的测试的输出平时不显示，
/// CI 日志里要看得出这几个测试是跳过了，而不是真的跑过了。
pub fn skip(reason: &str) {
    let test = thread::current()
        .name()
        .unwrap_or("Wayland 前端黑盒测试")
        .to_owned();
    let note = format!("跳过 {test}：{reason}\n");
    let _ = std::io::stderr().write_all(note.as_bytes());
}
