//! 对着私有总线上的假 portal 走一遍真的 D-Bus 往来：先订信号再读初值、第 1 版退回 `Read`、不按需拉起 portal、它后起来也接得上。
//! 要 dbus-daemon，缺省不跑：`cargo test -p qingjian-linux-wayland -- --ignored portal`。
//! 总线用自己的配置，服务目录里只有一个记下「被拉起过」的假服务，不会拉起本机真的 portal，也不碰会话总线。

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use zbus::Connection;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{OwnedValue, Value};

use super::super::{DESTINATION, Desktop, PATH, Preference, follow};

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

/// 假的 portal 服务：总线要是按需拉起它，它只留下一个 `activated` 文件。
const SERVICE: &str = "[D-BUS Service]
Name=org.freedesktop.portal.Desktop
Exec=/bin/sh -c 'echo > DIR/activated'
";

/// 等偏好落定最多等多久。
const PATIENCE: Duration = Duration::from_secs(5);

/// 一条私有总线（dbus-daemon 子进程），drop 时收掉。
struct Bus {
    daemon: Child,

    dir: PathBuf,

    address: String,
}

impl Bus {
    /// 没装 dbus-daemon 时为 `None`。
    fn start(name: &str) -> Option<Self> {
        let dir =
            std::env::temp_dir().join(format!("qingjian-portal-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let config = dir.join("bus.conf");
        let path = dir.to_string_lossy();
        std::fs::write(&config, CONFIG.replace("DIR", &path)).unwrap();
        let services = dir.join("services");
        std::fs::create_dir_all(&services).unwrap();
        std::fs::write(
            services.join("org.freedesktop.portal.Desktop.service"),
            SERVICE.replace("DIR", &path),
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

    /// 总线按需拉起过 portal。
    fn activated(&self) -> bool {
        self.dir.join("activated").exists()
    }

    async fn connect(&self) -> Connection {
        zbus::connection::Builder::address(self.address.as_str())
            .unwrap()
            .build()
            .await
            .unwrap()
    }

    /// 占上 portal 的名字、挂上假的 Settings。
    async fn serve<I: zbus::object_server::Interface>(&self, settings: I) -> Connection {
        zbus::connection::Builder::address(self.address.as_str())
            .unwrap()
            .name(DESTINATION)
            .unwrap()
            .serve_at(PATH, settings)
            .unwrap()
            .build()
            .await
            .unwrap()
    }

    /// 在这条总线上起跟随，像 `watch` 那样一直跑。
    async fn follow(&self, desktop: &Arc<Desktop>) -> tokio::task::JoinHandle<()> {
        let connection = self.connect().await;
        let desktop = Arc::clone(desktop);
        tokio::spawn(async move {
            follow(&connection, &desktop).await.unwrap();
        })
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// 第 2 版的 portal，像 GNOME 系后端：深浅之外还转出 GNOME 的动画开关，没有 `reduced-motion`。
struct Settings {
    scheme: u32,

    animations: bool,
}

#[zbus::interface(name = "org.freedesktop.portal.Settings")]
impl Settings {
    async fn read_one(&self, namespace: &str, key: &str) -> zbus::fdo::Result<OwnedValue> {
        match (namespace, key) {
            ("org.freedesktop.appearance", "color-scheme") => Ok(self.scheme.into()),
            ("org.gnome.desktop.interface", "enable-animations") => Ok(self.animations.into()),
            _ => Err(zbus::fdo::Error::Failed(
                "Requested setting not found".to_owned(),
            )),
        }
    }

    #[zbus(signal)]
    async fn setting_changed(
        emitter: &SignalEmitter<'_>,
        namespace: &str,
        key: &str,
        value: Value<'_>,
    ) -> zbus::Result<()>;
}

/// 第 1 版的 portal：只有 `Read`，值多包一层 variant。
struct OldSettings {
    scheme: u32,
}

#[zbus::interface(name = "org.freedesktop.portal.Settings")]
impl OldSettings {
    async fn read(&self, namespace: &str, key: &str) -> zbus::fdo::Result<OwnedValue> {
        match (namespace, key) {
            ("org.freedesktop.appearance", "color-scheme") => {
                Ok(Value::Value(Box::new(Value::U32(self.scheme)))
                    .try_into_owned()
                    .unwrap())
            }
            _ => Err(zbus::fdo::Error::Failed(
                "Requested setting not found".to_owned(),
            )),
        }
    }
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

/// 等到偏好变成 `expected`，等不到就失败。
async fn settle(desktop: &Desktop, expected: Preference) {
    let started = Instant::now();
    while desktop.preference() != expected {
        assert!(
            started.elapsed() < PATIENCE,
            "等不到 {expected:?}，现在是 {:?}",
            desktop.preference()
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn change(portal: &Connection, namespace: &str, key: &str, value: Value<'_>) {
    let emitter = SignalEmitter::new(portal, PATH).unwrap();
    Settings::setting_changed(&emitter, namespace, key, value)
        .await
        .unwrap();
}

#[test]
#[ignore = "要 dbus-daemon"]
fn reads_and_follows_a_portal() {
    let Some(bus) = Bus::start("follow") else {
        return;
    };
    runtime().block_on(async {
        let portal = bus
            .serve(Settings {
                scheme: 1,
                animations: false,
            })
            .await;
        let desktop = Arc::new(Desktop::default());
        let following = bus.follow(&desktop).await;
        settle(
            &desktop,
            Preference {
                dark: true,
                reduce_motion: true,
            },
        )
        .await;
        let appearance = "org.freedesktop.appearance";
        change(&portal, appearance, "color-scheme", Value::U32(2)).await;
        settle(
            &desktop,
            Preference {
                dark: false,
                reduce_motion: true,
            },
        )
        .await;
        // 不认得的键跳过，后面的照常
        change(&portal, appearance, "contrast", Value::U32(1)).await;
        change(&portal, appearance, "reduced-motion", Value::U32(1)).await;
        change(
            &portal,
            "org.gnome.desktop.interface",
            "enable-animations",
            Value::Bool(true),
        )
        .await;
        change(&portal, appearance, "color-scheme", Value::U32(1)).await;
        settle(
            &desktop,
            Preference {
                dark: true,
                reduce_motion: true,
            },
        )
        .await;
        change(&portal, appearance, "reduced-motion", Value::U32(0)).await;
        settle(
            &desktop,
            Preference {
                dark: true,
                reduce_motion: false,
            },
        )
        .await;
        assert!(!following.is_finished());
        following.abort();
    });
}

#[test]
#[ignore = "要 dbus-daemon"]
fn falls_back_to_read_on_a_version_one_portal() {
    let Some(bus) = Bus::start("old") else {
        return;
    };
    runtime().block_on(async {
        let _portal = bus.serve(OldSettings { scheme: 1 }).await;
        let desktop = Arc::new(Desktop::default());
        let following = bus.follow(&desktop).await;
        settle(
            &desktop,
            Preference {
                dark: true,
                reduce_motion: false,
            },
        )
        .await;
        following.abort();
    });
}

/// 读初值时 portal 还不在：不拉起它，按浅色；它后来上线了就读一遍，之后的变化照样收得到。
#[test]
#[ignore = "要 dbus-daemon"]
fn waits_for_a_portal_that_starts_later() {
    let Some(bus) = Bus::start("later") else {
        return;
    };
    runtime().block_on(async {
        let desktop = Arc::new(Desktop::default());
        let following = bus.follow(&desktop).await;
        // 让跟随那边订好信号、读完（读不到）
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(desktop.preference(), Preference::default());
        assert!(!bus.activated(), "读的时候拉起了 portal");
        let portal = bus
            .serve(Settings {
                scheme: 1,
                animations: false,
            })
            .await;
        settle(
            &desktop,
            Preference {
                dark: true,
                reduce_motion: true,
            },
        )
        .await;
        change(
            &portal,
            "org.freedesktop.appearance",
            "color-scheme",
            Value::U32(0),
        )
        .await;
        settle(
            &desktop,
            Preference {
                dark: false,
                reduce_motion: true,
            },
        )
        .await;
        assert!(!following.is_finished());
        following.abort();
        assert!(!bus.activated());
    });
}
