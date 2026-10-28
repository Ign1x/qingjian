//! 对着私有总线上的假 portal 走一遍真的 D-Bus 往来：先订信号再读初值、第 1 版退回 `Read`、不按需拉起 portal、它后起来也接得上。
//! 要 dbus-daemon，缺省不跑：`cargo test -p qingjian-linux-wayland -- --ignored portal`。
//! 总线见 [`crate::testing::Bus`]：自己的配置，服务目录里只有一个记下「被拉起过」的假服务，不会拉起本机真的 portal，也不碰会话总线。

use std::sync::Arc;
use std::time::{Duration, Instant};

use zbus::Connection;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{OwnedValue, Value};

use super::super::{DESTINATION, Desktop, PATH, Preference, follow};
use crate::testing::Bus;

/// 等偏好落定最多等多久。
const PATIENCE: Duration = Duration::from_secs(5);

/// 起一条只服务假 portal 的私有总线：服务目录里的假服务就叫 portal 的名字，被拉起过看得出来。
fn private_bus(name: &str) -> Option<Bus> {
    Bus::start(&format!("portal-{name}"), DESTINATION)
}

/// 在这条总线上起跟随，像 `watch` 那样一直跑。
async fn follow_on(bus: &Bus, desktop: &Arc<Desktop>) -> tokio::task::JoinHandle<()> {
    let connection = bus.connect().await;
    let desktop = Arc::clone(desktop);
    tokio::spawn(async move {
        follow(&connection, &desktop).await.unwrap();
    })
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
    let Some(bus) = private_bus("follow") else {
        return;
    };
    runtime().block_on(async {
        let portal = bus
            .serve(
                DESTINATION,
                PATH,
                Settings {
                    scheme: 1,
                    animations: false,
                },
            )
            .await;
        let desktop = Arc::new(Desktop::default());
        let following = follow_on(&bus, &desktop).await;
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
    let Some(bus) = private_bus("old") else {
        return;
    };
    runtime().block_on(async {
        let _portal = bus
            .serve(DESTINATION, PATH, OldSettings { scheme: 1 })
            .await;
        let desktop = Arc::new(Desktop::default());
        let following = follow_on(&bus, &desktop).await;
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
    let Some(bus) = private_bus("later") else {
        return;
    };
    runtime().block_on(async {
        let desktop = Arc::new(Desktop::default());
        let following = follow_on(&bus, &desktop).await;
        // 让跟随那边订好信号、读完（读不到）
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(desktop.preference(), Preference::default());
        assert!(!bus.activated(), "读的时候拉起了 portal");
        let portal = bus
            .serve(
                DESTINATION,
                PATH,
                Settings {
                    scheme: 1,
                    animations: false,
                },
            )
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
