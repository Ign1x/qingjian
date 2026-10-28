//! 拼音行显示声调：窗口里的拼音行另给带调的一串，应用里的行内拼音不变。

use qingjian_core::Engine;
use qingjian_dictionary::Dictionary;
use qingjian_platform::protocol::{
    ClientMessage, Frame, KeyEvent, KeyModifiers, PROTOCOL_VERSION, PreeditSegment, ServerMessage,
    SessionId,
};
use qingjian_platform::{HighlightKeys, KeyHintMode, LayoutMode, Scheme};

use crate::dispatch::{Router, RouterConfig};
use serde_json::json;

fn router(show_tones: bool) -> Router {
    let mut engine = Engine::new(
        Dictionary::parse(concat!(
            "你好\tni hao\t100\n你\tni\t90\n泥\tni\t80\n",
            "尼\tni\t70\n拟\tni\t60\n倪\tni\t50\n",
            "逆\tni\t40\n腻\tni\t30\n妮\tni\t20\n",
        ))
        .unwrap(),
    );
    engine.set_show_tones(show_tones);
    let mut router = Router::new(engine, RouterConfig::default());
    router.handle(ClientMessage::OpenSession {
        session: SessionId(1),
        app: None,
        protocol: PROTOCOL_VERSION,
    });
    router
}

fn press(router: &mut Router, code: u32, character: Option<char>) -> Frame {
    match router.handle(ClientMessage::Key {
        session: SessionId(1),
        event: KeyEvent::new(code, character, KeyModifiers::default()),
    }) {
        Some(ServerMessage::KeyResult { frame, .. }) => frame,
        other => panic!("按键要回 KeyResult：{other:?}"),
    }
}

fn compose(router: &mut Router, text: &str) -> Frame {
    text.chars()
        .map(|c| press(router, c as u32, Some(c)))
        .last()
        .unwrap()
}

fn joined(segments: &[PreeditSegment]) -> String {
    segments
        .iter()
        .map(|segment| segment.text.as_str())
        .collect()
}

#[test]
fn toned_pinyin_goes_to_the_window_row_only() {
    let frame = compose(&mut router(true), "nihao");
    assert_eq!(joined(&frame.preedit), "ni'hao");
    assert_eq!(frame.cursor, 6);
    let (segments, cursor) = *frame.window_preedit.expect("开着声调时窗口另给一串");
    assert_eq!(joined(&segments), "nǐ'hǎo");
    assert_eq!(cursor, 6);
}

/// 窗口那一串跟着高亮候选的读音走：高亮挪到单字「你」时只有第一个音节标调。
#[test]
fn window_row_follows_the_highlight() {
    let mut router = router(true);
    compose(&mut router, "nihao");
    let frame = press(&mut router, 0x28, None);
    assert_eq!(frame.candidates.items[frame.highlight].text, "你");
    let (segments, _) = *frame.window_preedit.expect("高亮单字也读得出声调");
    assert_eq!(joined(&segments), "nǐ'hao");
}

#[test]
fn no_window_row_without_tones() {
    let frame = compose(&mut router(false), "nihao");
    assert_eq!(joined(&frame.preedit), "ni'hao");
    assert_eq!(frame.window_preedit, None);
}

/// 矩阵滚动后帧内高亮与整份候选的下标不同，声调仍跟着实际选中的词，键盘也随同一帧返回。
#[test]
fn scrolled_grid_keeps_toned_pinyin_and_key_hint() {
    let mut router = router(true);
    let scheme = qingjian_core::shuangpin::Scheme::Xiaohe;
    router.engine.set_shuangpin(Some(scheme));
    router.config.scheme = Scheme::Shuangpin(scheme);
    router.config.page_size = 1;
    router.config.layout = LayoutMode::Horizontal;
    router.config.horizontal_grid = true;
    router.config.key_hint = KeyHintMode::Always;
    router.handle_linux(json!({"DisplayReporting": {"session": 1,
        "identity": {"generation": 1, "context": "tones-grid", "revision": 0},
        "features": {"grid": true, "key_hint": true}
    }}));
    compose(&mut router, "ni");
    for _ in 0..router.candidate_count() {
        if router.layout_candidate(router.highlight).unwrap().text == "妮" {
            break;
        }
        press(&mut router, 0x28, None);
    }
    let frame = router.current_frame();
    assert_eq!(frame.candidates.items[frame.highlight].text, "妮");
    assert!(router.highlight > frame.highlight, "矩阵视口已滚动");
    assert_eq!(frame.columns, 1);
    assert_eq!(joined(&frame.preedit), "ni");
    assert_eq!(joined(&frame.window_preedit.unwrap().0), "nī");

    let reply = router
        .handle_linux(json!({"Poll": {"session": 1}}))
        .unwrap();
    assert_eq!(reply["Update"]["frame"]["columns"], 1);
    assert!(reply["Update"]["key_hint"]["content"]["Shuangpin"].is_object());
    let repeated = router
        .handle_linux(json!({"Poll": {"session": 1}}))
        .unwrap();
    assert_eq!(repeated["Update"]["identity"], reply["Update"]["identity"]);
}

#[test]
fn pinyin_focus_keeps_tones_when_candidate_highlight_is_hidden() {
    let mut router = router(true);
    router.config.layout = LayoutMode::Horizontal;
    router.config.highlight_keys = HighlightKeys::LeftRight;
    compose(&mut router, "nihao");
    let frame = press(&mut router, 0x26, None);
    assert_eq!(frame.highlight, usize::MAX);
    assert_eq!(joined(&frame.preedit), "ni'hao");
    assert_eq!(joined(&frame.window_preedit.unwrap().0), "nǐ'hǎo");
    assert_eq!(press(&mut router, 0x28, None).highlight, 0);
}
