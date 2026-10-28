//! 一次黑盒测试的全套：临时目录、假合成器、假 Server 与被测进程，以及几个常用的步骤和收尾时的检查。

use std::collections::BTreeMap;
use std::thread;

use super::{Compositor, DISPLAY, Field, Frontend, Request, Sandbox, Server, Setup};

/// 假 Server 的 socket 名（在沙箱里），被测进程的 QINGJIAN_SOCKET。
const SERVER: &str = "qingjian.sock";

/// 字段按声明的先后丢弃：先杀被测进程，再停假 Server 与假合成器，最后删临时目录。
pub struct Rig {
    pub frontend: Frontend,

    pub server: Server,

    pub compositor: Compositor,

    _sandbox: Sandbox,

    /// 收过尾了（[`Self::stop`]）。
    stopped: bool,
}

impl Rig {
    /// 起好合成器与 Server，再起被测进程，不等它就绪。
    pub fn spawn(setup: Setup) -> Self {
        let sandbox = Sandbox::new();
        let compositor = Compositor::start(sandbox.root(), setup);
        let server = Server::start(&sandbox.path(SERVER));
        let frontend = Frontend::spawn(&sandbox, DISPLAY, &sandbox.path(SERVER));
        Self {
            frontend,
            server,
            compositor,
            _sandbox: sandbox,
            stopped: false,
        }
    }

    /// 起好并等被测进程就绪；本机缺运行库时 `None`（已打印跳过说明）。
    pub fn start(setup: Setup) -> Option<Self> {
        let mut rig = Self::spawn(setup);
        rig.frontend.ready().then_some(rig)
    }

    /// 输入框激活：等被测进程接管键盘，并把 grab 给的键位表转给虚拟键盘（之后转键才合法）。
    pub fn focus(&self) {
        self.focus_with(Field::default());
    }

    /// 同 [`Self::focus`]，输入框报这些状态（前后文、内容类型）。
    pub fn focus_with(&self, field: Field) {
        let mark = self.compositor.requests.len();
        self.compositor.activate_with(field);
        self.compositor
            .requests
            .wait(mark, "接管键盘", |request| *request == Request::Grab);
        self.compositor
            .requests
            .wait(mark, "虚拟键盘的键位表", |request| {
                matches!(request, Request::Keymap(_))
            });
    }

    /// 收尾：发 SIGTERM，被测进程要以 0 退出、日志里没有 panic 与 ERROR，退出前交还键盘、补齐转出去的抬键、收起候选窗，
    /// 与合成器、Server 的连接都断开；之后核对没有违规。测试自己收过退出状态（起不来的那几个、SIGTERM 那个）时只核对违规。
    /// 没调过的话 drop 时自动调；要在收尾之后再查记录的测试自己调。
    pub fn stop(&mut self) {
        if std::mem::replace(&mut self.stopped, true) {
            return;
        }
        if let Some(exit) = self.frontend.finish() {
            assert_eq!(
                exit.code,
                Some(0),
                "SIGTERM 之后要以 0 退出：\n{}",
                exit.stderr
            );
            let bad: Vec<&str> = exit
                .stderr
                .lines()
                .filter(|line| line.contains(" ERROR ") || line.contains("panicked"))
                .collect();
            assert!(bad.is_empty(), "日志里有错：{bad:#?}");
            self.compositor
                .requests
                .wait(0, "被测进程断开 Wayland 连接", |request| {
                    matches!(request, Request::Disconnected(_))
                });
            leftovers(&self.compositor.requests.entries());
            self.server
                .connections
                .settle("被测进程断开与 Server 的连接", |entries| {
                    let opened = entries.iter().filter(|opened| **opened).count();
                    opened * 2 == entries.len()
                });
        }
        let violations: Vec<Request> = self
            .compositor
            .requests
            .entries()
            .into_iter()
            .filter(|request| {
                matches!(
                    request,
                    Request::Violation(_) | Request::Disconnected(Some(_))
                )
            })
            .collect();
        assert!(violations.is_empty(), "被测进程违反了协议：{violations:#?}");
        let violations = self.server.violations.entries();
        assert!(
            violations.is_empty(),
            "假 Server 收到了没想到的消息：{violations:#?}"
        );
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        if thread::panicking() {
            // 测试已经失败：假 Server 那边的违规常常就是原因，一起打出来
            let violations = self.server.violations.entries();
            if !violations.is_empty() {
                eprintln!("假 Server 收到的没想到的消息：{violations:#?}");
            }
            return;
        }
        self.stop();
    }
}

/// 被测进程退出时不能留下的：拿着键盘没还、虚拟键盘上还按着的键（应用里就一直按着）、挂着缓冲的候选窗。
/// 没见过按下的抬起不算（grab 之前就按着的键，抬起只转发）。
fn leftovers(requests: &[Request]) {
    let grab = requests
        .iter()
        .rev()
        .find(|request| matches!(request, Request::Grab | Request::Release));
    assert_ne!(grab, Some(&Request::Grab), "退出前没交还键盘");
    let mut held: BTreeMap<u32, i32> = BTreeMap::new();
    for request in requests {
        if let Request::Key { code, pressed, .. } = request {
            *held.entry(*code).or_default() += if *pressed { 1 } else { -1 };
        }
    }
    held.retain(|_, count| *count > 0);
    assert!(held.is_empty(), "退出前虚拟键盘上还按着这些键：{held:?}");
    let popup = requests.iter().rev().find_map(|request| match request {
        Request::Popup { picture, .. } => Some(*picture),
        _ => None,
    });
    assert!(
        popup.is_none_or(|picture| picture.is_none()),
        "退出前候选窗还挂着缓冲"
    );
}
