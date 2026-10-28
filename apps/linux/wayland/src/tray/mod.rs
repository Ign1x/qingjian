//! 托盘：StatusNotifierItem 加 com.canonical.dbusmenu，图标显示中 / 英，菜单里切中 / 英、全角标点、简繁，打开设置。
//! 后台线程上一个单线程 tokio 运行时跑 zbus（与 `crate::portal` 同一个路子）；主线程经 [`Tray`] 交模式、收托盘上点出来的切换，
//! 托盘线程排了切换就往一个 socket 里写一个字节，把在 `poll` 里等合成器事件的主循环叫醒。设计见 `docs/design/linux-wayland.md`「托盘」。

mod clicks;
mod handle;
mod icons;
mod item;
mod menu;
mod settings;

#[cfg(test)]
mod tests;

pub use handle::Tray;

use std::os::unix::net::UnixStream;
use std::sync::{Arc, mpsc};
use std::time::Duration;

use futures_util::future::Either;
use futures_util::{StreamExt, stream};
use qingjian_platform::protocol::linux::InputMode;
use tokio::sync::watch;
use zbus::proxy::{CacheProperties, MethodFlags};
use zbus::{Connection, Proxy};

use clicks::Clicks;
use icons::Icons;
use item::Item;
use menu::Menu;

/// 图标对象的路径：托盘宿主登记时只给了总线名，按约定到这里找。
const ITEM_PATH: &str = "/StatusNotifierItem";

/// 菜单对象的路径（图标的 `Menu` 属性指过来）。
const MENU_PATH: &str = "/MenuBar";

/// 托盘宿主（waybar 这类状态栏）挂的登记处。
const WATCHER: &str = "org.kde.StatusNotifierWatcher";

const WATCHER_PATH: &str = "/StatusNotifierWatcher";

/// 登记处不在时总线回这两种错误。
const ABSENT: [&str; 2] = [
    "org.freedesktop.DBus.Error.ServiceUnknown",
    "org.freedesktop.DBus.Error.NameHasNoOwner",
];

/// 登记处卡住时一次登记最多等多久（libdbus 与 GDBus 的缺省超时）。
const TIMEOUT: Duration = Duration::from_secs(25);

/// 起托盘线程，不等它连上总线就返回。没有会话总线时托盘线程记一句日志就结束，主线程这一头照常收发（什么都收不到）。
pub fn spawn() -> Tray {
    let (modes, receiver) = watch::channel(None);
    let (sender, clicks) = mpsc::channel();
    let (wake, waker) = match UnixStream::pair() {
        Ok(pair) => pair,
        Err(error) => {
            tracing::warn!(%error, "建不了唤醒主循环的 socket，不放托盘图标");
            return Tray::new(modes, clicks, None);
        }
    };
    // 两头都不阻塞：托盘线程写不进（主循环没来得及读）就算了，主循环读空了就停
    if let Err(error) = wake
        .set_nonblocking(true)
        .and_then(|()| waker.set_nonblocking(true))
    {
        tracing::warn!(%error, "唤醒 socket 设不了非阻塞，不放托盘图标");
        return Tray::new(modes, clicks, None);
    }
    let tray = Tray::new(modes, clicks, Some(wake));
    let clicks = Clicks::new(sender, Arc::new(waker));
    let spawned = std::thread::Builder::new()
        .name("qingjian-tray".to_owned())
        .spawn(move || {
            if let Err(error) = run(receiver, clicks) {
                tracing::info!(%error, "托盘停了（没有会话总线？），中 / 英只看候选窗位置的提示");
            }
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "起不了托盘线程，不放托盘图标");
    }
    tray
}

/// 连会话总线，挂上图标与菜单，跟着模式与托盘宿主直到连接断开。单线程运行时：zbus 的 tokio 后端把收发挂在当前运行时上。
fn run(modes: watch::Receiver<Option<InputMode>>, clicks: Clicks) -> zbus::Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let connection = zbus::connection::Builder::session()?
            .method_timeout(TIMEOUT)
            .build()
            .await?;
        serve(&connection, modes, clicks).await
    })
}

/// 在这条连接上挂图标与菜单、占上 `org.kde.StatusNotifierItem-<pid>-1`、向托盘宿主登记；之后模式变了就换图标与菜单的勾，
/// 托盘宿主上线或重启就再登记一次（它不在时不报错，等它上线）。登记不让总线按需拉起什么：登记处都是状态栏自己挂的。
async fn serve(
    connection: &Connection,
    mut modes: watch::Receiver<Option<InputMode>>,
    clicks: Clicks,
) -> zbus::Result<()> {
    let mode = modes.borrow_and_update().clone();
    let server = connection.object_server();
    server
        .at(
            ITEM_PATH,
            Item::new(mode.clone(), clicks.clone(), Icons::new()),
        )
        .await?;
    server.at(MENU_PATH, Menu::new(mode, clicks)).await?;
    let name = format!("org.kde.StatusNotifierItem-{}-1", std::process::id());
    connection.request_name(name.as_str()).await?;
    let watcher: Proxy<'_> = zbus::proxy::Builder::new(connection)
        .destination(WATCHER)?
        .path(WATCHER_PATH)?
        .interface(WATCHER)?
        .cache_properties(CacheProperties::No)
        .build()
        .await?;
    let owners = watcher.receive_owner_changed().await?.map(Either::Left);
    // 主线程那一头没了（进程在退出）流就结束；托盘宿主的上下线照样跟着，直到连接断开
    let changes = stream::unfold(modes, |mut modes| async move {
        modes.changed().await.ok()?;
        let mode = modes.borrow_and_update().clone();
        Some((mode, modes))
    })
    .map(Either::Right);
    let mut events = std::pin::pin!(stream::select(owners, changes));
    register(&watcher, &name).await;
    while let Some(event) = events.next().await {
        match event {
            Either::Left(Some(_)) => register(&watcher, &name).await,
            Either::Left(None) => tracing::debug!("托盘宿主退出了，等它回来再登记"),
            Either::Right(mode) => {
                if let Err(error) = update(connection, mode).await {
                    tracing::debug!(%error, "托盘图标没换上");
                }
            }
        }
    }
    tracing::info!("会话总线断开，托盘图标不再更新");
    Ok(())
}

/// 向托盘宿主登记；它不在就记一句，等它上线（`NameOwnerChanged`）再登记。
async fn register(watcher: &Proxy<'_>, name: &str) {
    let body = (name,);
    let call = watcher.call_with_flags::<_, _, ()>(
        "RegisterStatusNotifierItem",
        MethodFlags::NoAutoStart.into(),
        &body,
    );
    match tokio::time::timeout(TIMEOUT, call).await {
        Ok(Ok(_)) => tracing::info!("托盘图标已登记"),
        Ok(Err(zbus::Error::MethodError(error, ..)))
            if ABSENT.iter().any(|absent| error == *absent) =>
        {
            tracing::info!("没有托盘宿主（StatusNotifierWatcher 不在），等它上线再登记");
        }
        Ok(Err(error)) => tracing::info!(%error, "托盘图标登记失败，等托盘宿主重启再登记"),
        Err(_) => tracing::info!("托盘宿主没有回应，等它重启再登记"),
    }
}

/// 换上新模式：图标与提示变了就发 `NewIcon` / `NewToolTip`（托盘宿主收到再来读属性），菜单的勾变了就发 `LayoutUpdated`。
async fn update(connection: &Connection, mode: Option<InputMode>) -> zbus::Result<()> {
    let server = connection.object_server();
    let item = server.interface::<_, Item>(ITEM_PATH).await?;
    let (icon, tooltip) = item.get_mut().await.set(mode.clone());
    if icon {
        Item::new_icon(item.signal_emitter()).await?;
    }
    if tooltip {
        Item::new_tool_tip(item.signal_emitter()).await?;
    }
    let menu = server.interface::<_, Menu>(MENU_PATH).await?;
    let revision = menu.get_mut().await.set(mode);
    if let Some(revision) = revision {
        Menu::layout_updated(menu.signal_emitter(), revision, 0).await?;
    }
    Ok(())
}
