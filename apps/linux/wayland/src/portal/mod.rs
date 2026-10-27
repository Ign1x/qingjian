//! 桌面的外观偏好：后台线程经会话总线读 xdg-desktop-portal 的 Settings（深浅、减少动态效果），跟着 `SettingChanged` 更新。
//! 画候选窗时只读 [`Desktop`] 里的原子量，不碰 D-Bus；没有会话总线或 portal 时一直是缺省偏好（浅色、照常播动画）。
//! 设计见 `docs/design/linux-wayland.md`「显示」。

mod desktop;
mod preference;
mod setting;

#[cfg(test)]
mod tests;

pub use desktop::Desktop;
pub use preference::Preference;
pub use setting::Setting;

use std::sync::Arc;
use std::time::Duration;

use futures_util::future::Either;
use futures_util::{StreamExt, stream};
use zbus::proxy::MethodFlags;
use zbus::zvariant::{OwnedValue, Value};
use zbus::{Connection, Proxy};

const DESTINATION: &str = "org.freedesktop.portal.Desktop";

const PATH: &str = "/org/freedesktop/portal/desktop";

const INTERFACE: &str = "org.freedesktop.portal.Settings";

/// 第 1 版的 Settings 没有 `ReadOne`，调它回这个错误。
const UNKNOWN_METHOD: &str = "org.freedesktop.DBus.Error.UnknownMethod";

/// portal 卡住时一次读最多等多久（libdbus 与 GDBus 的缺省超时）；超时了也接着等变化的信号。
const TIMEOUT: Duration = Duration::from_secs(25);

/// 起后台线程跟着桌面的外观偏好，不等读到就返回：总线或 portal 再慢也不耽误启动。
pub fn watch() -> Arc<Desktop> {
    let desktop = Arc::new(Desktop::default());
    let shared = Arc::clone(&desktop);
    let spawned = std::thread::Builder::new()
        .name("qingjian-portal".to_owned())
        .spawn(move || {
            if let Err(error) = run(&shared) {
                tracing::info!(%error, "读不到桌面的外观设置，候选窗外观「跟随系统」按浅色");
            }
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "起不了读桌面外观的线程，候选窗外观「跟随系统」按浅色");
    }
    desktop
}

/// 连会话总线，跟着 portal 直到连接断开。单线程运行时：zbus 的 tokio 后端把收发挂在当前运行时上，不再另起线程。
fn run(desktop: &Desktop) -> zbus::Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let connection = zbus::connection::Builder::session()?
            .method_timeout(TIMEOUT)
            .build()
            .await?;
        follow(&connection, desktop).await
    })
}

/// 先订 `SettingChanged` 与 portal 的上下线，再读初值（读的途中改了也漏不掉），之后跟着信号更新，直到连接断开。
/// 读的时候不让总线按需拉起 portal：输入法在会话里起得早，这时环境变量（`WAYLAND_DISPLAY`）可能还没交给 D-Bus 的激活环境，
/// 拉起来的 portal 后端连不上合成器。portal 还没起来就等它上线再读。
async fn follow(connection: &Connection, desktop: &Desktop) -> zbus::Result<()> {
    let proxy = Proxy::new(connection, DESTINATION, PATH, INTERFACE).await?;
    let changes = proxy
        .receive_signal("SettingChanged")
        .await?
        .map(Either::Left);
    let owners = proxy.receive_owner_changed().await?.map(Either::Right);
    let mut events = stream::select(changes, owners);
    read_all(&proxy, desktop).await;
    while let Some(event) = events.next().await {
        match event {
            Either::Left(message) => {
                let body = message.body();
                match body.deserialize::<(&str, &str, Value<'_>)>() {
                    Ok((namespace, key, value)) => {
                        if let Some(setting) = record(desktop, namespace, key, &value) {
                            tracing::info!(?setting, "桌面的外观偏好变了");
                        }
                    }
                    Err(error) => tracing::debug!(%error, "SettingChanged 的参数解不开"),
                }
            }
            // portal 上线（含重启）：重读一遍，它不在的时候可能改过
            Either::Right(Some(_)) => read_all(&proxy, desktop).await,
            Either::Right(None) => tracing::debug!("xdg-desktop-portal 退出了，外观偏好保持不变"),
        }
    }
    tracing::info!("会话总线断开，候选窗外观不再跟着桌面变");
    Ok(())
}

/// 读关心的几项；深浅读不到时记一条 info，按浅色。
async fn read_all(proxy: &Proxy<'_>, desktop: &Desktop) {
    let scheme = read(proxy, desktop, Setting::COLOR_SCHEME).await;
    for (namespace, key) in [Setting::REDUCED_MOTION, Setting::ENABLE_ANIMATIONS] {
        if let Err(error) = read(proxy, desktop, (namespace, key)).await {
            // 只有部分桌面有，没有是常事
            tracing::debug!(%error, namespace, key, "portal 没有这项设置");
        }
    }
    match scheme {
        Ok(()) => {
            let preference = desktop.preference();
            tracing::info!(
                dark = preference.dark,
                reduce_motion = preference.reduce_motion,
                "已读到桌面的外观偏好"
            );
        }
        Err(error) => tracing::info!(
            %error,
            "xdg-desktop-portal 给不出深浅偏好，候选窗外观「跟随系统」按浅色"
        ),
    }
}

/// 读一项记下来。第 1 版的 Settings 没有 `ReadOne`，退回 `Read`（值多包一层 variant，[`Setting::parse`] 剥掉）。
async fn read(
    proxy: &Proxy<'_>,
    desktop: &Desktop,
    (namespace, key): (&str, &str),
) -> zbus::Result<()> {
    let body = &(namespace, key);
    let value = match call(proxy, "ReadOne", body).await {
        Err(zbus::Error::MethodError(name, ..)) if name == UNKNOWN_METHOD => {
            call(proxy, "Read", body).await?
        }
        result => result?,
    };
    record(desktop, namespace, key, &value);
    Ok(())
}

/// 调一次 portal，不按需拉起它（见 [`follow`]）：没起来时总线直接回 `ServiceUnknown`。
async fn call(
    proxy: &Proxy<'_>,
    method: &'static str,
    body: &(&str, &str),
) -> zbus::Result<OwnedValue> {
    proxy
        .call_with_flags(method, MethodFlags::NoAutoStart.into(), body)
        .await?
        .ok_or(zbus::Error::InvalidReply)
}

/// 记下 portal 给的一项；是认得的设置、且值变了时返回它。
fn record(desktop: &Desktop, namespace: &str, key: &str, value: &Value<'_>) -> Option<Setting> {
    let setting = Setting::parse(namespace, key, value)?;
    desktop.apply(setting).then_some(setting)
}
