//! 超过一屏的矩阵与按需云联想的交互；内存通道保证请求和回包的顺序。

use super::{
    enable_capabilities, poll, predict_combo_key, press_key, reply_with, setup_with_config,
};
use qingjian_linux_server::RouterConfig;
use qingjian_platform::LayoutMode;
use qingjian_platform::protocol::{KeyEvent, KeyModifiers};
use serde_json::json;

#[test]
fn manual_prediction_after_scrolling_grid_returns_to_first_page() {
    let (mut router, requests, responses) = setup_with_config(
        false,
        RouterConfig {
            page_size: 1,
            cloud_enabled: true,
            cloud_slots: 1,
            horizontal_grid: true,
            layout: LayoutMode::Horizontal,
            ..Default::default()
        },
    );
    enable_capabilities(&mut router);
    router.handle_linux(json!({"DisplayReporting": {
        "session": 1, "identity": {"generation": 1, "context": "grid", "revision": 0},
        "features": {"grid": true}
    }}));
    router
        .handle_linux(json!({"LinuxEvent": {"session": 1, "event": {"Focus": {"focused": true}}}}));
    for c in ['n', 'i'] {
        press_key(
            &mut router,
            KeyEvent::new(c as u32, Some(c), KeyModifiers::default()),
        );
    }
    for _ in 0..7 {
        press_key(
            &mut router,
            KeyEvent::new(0x28, None, KeyModifiers::default()),
        );
    }
    let frame = poll(&mut router);
    assert_eq!(frame.columns, 1);
    assert!(frame.page >= 6);
    assert_ne!(frame.candidates.items[0].text, "你");
    press_key(&mut router, predict_combo_key());
    let request = requests.try_recv().expect("按需请求已发送");
    assert!(request.manual);
    let frame = poll(&mut router);
    assert_eq!((frame.columns, frame.page, frame.highlight), (0, 0, 0));
    reply_with(&responses, request.sequence, "妮");
    assert_eq!(poll(&mut router).candidates.items[0].text, "妮");
}
