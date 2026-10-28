//! D-Bus 断言的等待与菜单读取工具。

use futures_util::StreamExt;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use zbus::proxy::CacheProperties;
use zbus::zvariant::{OwnedValue, Value};
use zbus::{Connection, Proxy};

const PATIENCE: Duration = Duration::from_secs(5);
type Layout = (u32, (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>));

pub(super) fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

fn item_name() -> String {
    format!("org.kde.StatusNotifierItem-{}-1", std::process::id())
}

/// 等到假托盘宿主收到登记，等不到就失败。
pub(super) async fn registered(registered: &Mutex<Vec<String>>) {
    let started = Instant::now();
    while !registered.lock().unwrap().contains(&item_name()) {
        assert!(started.elapsed() < PATIENCE, "等不到登记");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

pub(super) async fn proxy<'a>(
    connection: &'a Connection,
    path: &'a str,
    interface: &'a str,
) -> Proxy<'a> {
    zbus::proxy::Builder::new(connection)
        .destination(item_name())
        .unwrap()
        .path(path)
        .unwrap()
        .interface(interface)
        .unwrap()
        .cache_properties(CacheProperties::No)
        .build()
        .await
        .unwrap()
}

pub(super) async fn layout(menu: &Proxy<'_>) -> Layout {
    menu.call("GetLayout", &(0_i32, -1_i32, Vec::<String>::new()))
        .await
        .unwrap()
}

/// 第 `index` 个子节点的 toggle-state；没有为 `None`。
pub(super) fn toggle_state(layout: &Layout, index: usize) -> Option<i32> {
    let child = &layout.1.2[index];
    let structure = <&zbus::zvariant::Structure<'_>>::try_from(child).unwrap();
    let Value::Dict(properties) = &structure.fields()[1] else {
        panic!("子节点的属性不是字典");
    };
    properties.get::<_, i32>(&"toggle-state").unwrap()
}

pub(super) async fn next_signal(signals: &mut zbus::proxy::SignalStream<'_>, what: &str) {
    tokio::time::timeout(PATIENCE, signals.next())
        .await
        .unwrap_or_else(|_| panic!("等不到 {what}"))
        .unwrap();
}
