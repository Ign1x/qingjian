//! 一次黑盒测试的全套：临时目录、假合成器、假 Server 与被测进程，以及几个常用的步骤。

use std::thread;

use super::{Compositor, DISPLAY, Frontend, Request, Sandbox, Server, Setup};

/// 假 Server 的 socket 名（在沙箱里），被测进程的 QINGJIAN_SOCKET。
const SERVER: &str = "qingjian.sock";

/// 字段按声明的先后丢弃：先杀被测进程，再停假 Server 与假合成器，最后删临时目录。
pub struct Rig {
    pub frontend: Frontend,

    pub server: Server,

    pub compositor: Compositor,

    _sandbox: Sandbox,
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
        }
    }

    /// 起好并等被测进程就绪；本机缺运行库时 `None`（已打印跳过说明）。
    pub fn start(setup: Setup) -> Option<Self> {
        let mut rig = Self::spawn(setup);
        rig.frontend.ready().then_some(rig)
    }

    /// 输入框激活：等被测进程接管键盘，并把 grab 给的键位表转给虚拟键盘（之后转键才合法）。
    pub fn focus(&self) {
        let mark = self.compositor.requests.len();
        self.compositor.activate();
        self.compositor
            .requests
            .wait(mark, "接管键盘", |request| *request == Request::Grab);
        self.compositor
            .requests
            .wait(mark, "虚拟键盘的键位表", |request| {
                matches!(request, Request::Keymap(_))
            });
    }
}

impl Drop for Rig {
    /// 测完核对一遍：被测进程没做过真合成器会因此断开它的事。
    fn drop(&mut self) {
        if thread::panicking() {
            return;
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
    }
}
