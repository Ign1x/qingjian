//! 测试独占的会话总线，退出时回收，不读取宿主的总线地址。

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};

pub(super) struct Bus {
    child: Child,

    pub(super) address: String,
}

impl Bus {
    pub(super) fn start() -> Self {
        let child = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("需要 dbus-daemon 运行面板信号测试");
        let mut bus = Self {
            child,
            address: String::new(),
        };
        BufReader::new(bus.child.stdout.take().unwrap())
            .read_line(&mut bus.address)
            .unwrap();
        bus.address = bus.address.trim().to_owned();
        assert!(!bus.address.is_empty());
        bus
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
