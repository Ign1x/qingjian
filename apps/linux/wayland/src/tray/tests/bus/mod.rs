//! 对着私有总线上的假托盘宿主走一遍真的 D-Bus 往来：登记、图标的属性、菜单的布局、点菜单与左键单击排出切换、
//! 模式变了换图标与勾、托盘宿主后上线也登记得上、登记时不按需拉起它。
//! 要 dbus-daemon，缺省不跑：`cargo test -p qingjian-linux-wayland -- --ignored tray`。总线见 [`crate::testing::Bus`]。

use std::sync::{Arc, Mutex};
use std::time::Duration;

use qingjian_platform::protocol::linux::InputMode;
use zbus::zvariant::{Array, OwnedValue, Value};

use crate::tray::{ITEM_PATH, MENU_PATH, WATCHER, WATCHER_PATH};

mod support;
mod tray;
use support::{layout, next_signal, proxy, registered, runtime, toggle_state};
mod watcher;
use crate::session::ModeTarget;
use crate::testing::Bus;
use tray::Tray;
use watcher::Watcher;

#[test]
#[ignore = "要 dbus-daemon"]
fn registers_and_serves_the_icon_and_menu() {
    let Some(bus) = Bus::start("tray", WATCHER) else {
        return;
    };
    runtime().block_on(async {
        let names = Arc::new(Mutex::new(Vec::new()));
        let _watcher = bus
            .serve(
                WATCHER,
                WATCHER_PATH,
                Watcher {
                    registered: Arc::clone(&names),
                },
            )
            .await;
        let (tray, serving) = Tray::start(&bus).await;
        registered(&names).await;

        let client = bus.connect().await;
        let item = proxy(&client, ITEM_PATH, "org.kde.StatusNotifierItem").await;
        let menu = proxy(&client, MENU_PATH, "com.canonical.dbusmenu").await;
        assert_eq!(
            item.get_property::<String>("Status").await.unwrap(),
            "Active"
        );
        assert_eq!(item.get_property::<String>("Id").await.unwrap(), "qingjian");
        assert!(!item.get_property::<bool>("ItemIsMenu").await.unwrap());
        let path: OwnedValue = item.get_property("Menu").await.unwrap();
        assert_eq!(path.value_signature().to_string(), "o");
        assert_eq!(
            item.get_property::<String>("IconName").await.unwrap(),
            "qingjian",
            "不知道模式时用图标主题里的青简标志"
        );
        let logo: OwnedValue = item.get_property("IconPixmap").await.unwrap();
        assert_eq!(logo.value_signature().to_string(), "a(iiay)");
        assert_eq!(<&Array<'_>>::try_from(&logo).unwrap().len(), 6);
        let tooltip: OwnedValue = item.get_property("ToolTip").await.unwrap();
        assert_eq!(tooltip.value_signature().to_string(), "(sa(iiay)ss)");

        let before = layout(&menu).await;
        assert_eq!(before.1.0, 0);
        assert_eq!(before.1.2.len(), 9);
        assert_eq!(toggle_state(&before, 1), Some(0), "不知道模式时英文没选上");

        // 主线程换了模式：发 NewIcon / NewToolTip 与 LayoutUpdated，再读就是英文的图标与勾
        let mut icons = item.receive_signal("NewIcon").await.unwrap();
        let mut tooltips = item.receive_signal("NewToolTip").await.unwrap();
        let mut layouts = menu.receive_signal("LayoutUpdated").await.unwrap();
        tray.modes.send_replace(Some(InputMode {
            english: true,
            ..InputMode::default()
        }));
        next_signal(&mut icons, "NewIcon").await;
        next_signal(&mut tooltips, "NewToolTip").await;
        next_signal(&mut layouts, "LayoutUpdated").await;
        assert_eq!(item.get_property::<String>("IconName").await.unwrap(), "");
        let english: OwnedValue = item.get_property("IconPixmap").await.unwrap();
        assert_ne!(english, logo);
        let after = layout(&menu).await;
        assert!(after.0 > before.0, "版本号要往前走");
        assert_eq!(toggle_state(&after, 1), Some(1));

        // 点菜单里的「中文」与左键单击图标：都排给主线程，并叫醒它
        let () = menu
            .call("Event", &(1_i32, "clicked", Value::I32(0), 0_u32))
            .await
            .unwrap();
        assert_eq!(tray.clicked(), Some(ModeTarget::English(false)));
        let () = menu
            .call("Event", &(1_i32, "hovered", Value::I32(0), 0_u32))
            .await
            .unwrap();
        assert_eq!(tray.clicked(), None, "只认 clicked");
        let () = item.call("Activate", &(0_i32, 0_i32)).await.unwrap();
        assert_eq!(
            tray.clicked(),
            Some(ModeTarget::English(false)),
            "左键单击按画着的反过来要"
        );
        let unknown: Vec<i32> = menu
            .call(
                "EventGroup",
                &(vec![(42_i32, "clicked", Value::I32(0), 0_u32)],),
            )
            .await
            .unwrap();
        assert_eq!(unknown, [42]);
        assert!(!bus.activated());
        assert!(!serving.is_finished());
        serving.abort();
    });
}

/// 起来时还没有托盘宿主：不报错、不拉起它；它上线了就登记。
#[test]
#[ignore = "要 dbus-daemon"]
fn registers_with_a_host_that_starts_later_and_restarts() {
    let Some(bus) = Bus::start("tray-later", WATCHER) else {
        return;
    };
    runtime().block_on(async {
        let (_tray, serving) = Tray::start(&bus).await;
        // 让托盘那边挂好对象、登记（登记不上）
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(!bus.activated(), "登记时拉起了托盘宿主");
        assert!(!serving.is_finished(), "没有托盘宿主不算错");

        let names = Arc::new(Mutex::new(Vec::new()));
        let _watcher = bus
            .serve(
                WATCHER,
                WATCHER_PATH,
                Watcher {
                    registered: Arc::clone(&names),
                },
            )
            .await;
        registered(&names).await;
        // 换成另一条连接持有宿主名，模拟状态栏重启；旧连接保留，保证新宿主确实收到重新登记。
        _watcher.release_name(WATCHER).await.unwrap();
        let restarted = Arc::new(Mutex::new(Vec::new()));
        let _replacement = bus
            .serve(
                WATCHER,
                WATCHER_PATH,
                Watcher {
                    registered: Arc::clone(&restarted),
                },
            )
            .await;
        registered(&restarted).await;
        assert!(!bus.activated());
        serving.abort();
    });
}
