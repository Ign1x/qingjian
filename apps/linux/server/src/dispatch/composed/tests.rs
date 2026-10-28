//! 拼音行显示声调：窗口里的拼音行另给带调的一串，应用里的行内拼音不变。

use qingjian_core::Engine;
use qingjian_dictionary::Dictionary;
use qingjian_platform::protocol::{
    ClientMessage, Frame, KeyEvent, KeyModifiers, PROTOCOL_VERSION, PreeditSegment, ServerMessage,
    SessionId,
};

use crate::dispatch::{Router, RouterConfig};

fn router(show_tones: bool) -> Router {
    let mut engine =
        Engine::new(Dictionary::parse("你好\tni hao\t100\n你\tni\t90\n泥\tni\t80\n").unwrap());
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
