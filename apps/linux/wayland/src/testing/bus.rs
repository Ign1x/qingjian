//! 一条私有总线（dbus-daemon 子进程），drop 时收掉。用自己的配置，服务目录里只有一个假服务：
//! 总线要是按需拉起它，它只留下一个 `activated` 文件，不会拉起本机真的服务，也不碰会话总线。

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use zbus::Connection;
use zbus::object_server::Interface;

const CONFIG: &str = r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:path=DIR/bus</listen>
  <auth>EXTERNAL</auth>
  <servicedir>DIR/services</servicedir>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"#;

const SERVICE: &str = "[D-BUS Service]
Name=NAME
Exec=/bin/sh -c 'echo > DIR/activated'
";

pub struct Bus {
    daemon: Child,

    dir: PathBuf,

    address: String,
}

impl Bus {
    /// `name` 区分同一个进程里的几条总线；`service` 是那个只记下「被拉起过」的假服务的总线名。没装 dbus-daemon 时为 `None`。
    pub fn start(name: &str, service: &str) -> Option<Self> {
        let dir = std::env::temp_dir().join(format!("qingjian-bus-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let config = dir.join("bus.conf");
        let path = dir.to_string_lossy();
        std::fs::write(&config, CONFIG.replace("DIR", &path)).unwrap();
        let services = dir.join("services");
        std::fs::create_dir_all(&services).unwrap();
        std::fs::write(
            services.join(format!("{service}.service")),
            SERVICE.replace("NAME", service).replace("DIR", &path),
        )
        .unwrap();
        let mut daemon = Command::new("dbus-daemon")
            .arg(format!("--config-file={}", config.display()))
            .args(["--nofork", "--print-address=1"])
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .inspect_err(|error| eprintln!("dbus-daemon 起不来（{error}），跳过"))
            .ok()?;
        // 打印出地址时已经在听了
        let mut address = String::new();
        BufReader::new(daemon.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        let bus = Self {
            daemon,
            dir,
            address: address.trim().to_owned(),
        };
        assert!(!bus.address.is_empty(), "dbus-daemon 没给出地址");
        Some(bus)
    }

    /// 总线按需拉起过那个假服务。
    pub fn activated(&self) -> bool {
        self.dir.join("activated").exists()
    }

    pub async fn connect(&self) -> Connection {
        zbus::connection::Builder::address(self.address.as_str())
            .unwrap()
            .build()
            .await
            .unwrap()
    }

    /// 一条新连接：占上 `name`，在 `path` 挂上 `interface`。
    pub async fn serve<I: Interface>(&self, name: &str, path: &str, interface: I) -> Connection {
        zbus::connection::Builder::address(self.address.as_str())
            .unwrap()
            .name(name)
            .unwrap()
            .serve_at(path, interface)
            .unwrap()
            .build()
            .await
            .unwrap()
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
