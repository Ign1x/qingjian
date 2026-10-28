//! 云服务热加载与挂起会话的候选清理。

use super::{temp_dir, watched};
use qingjian_platform::protocol::SessionId;

#[test]
fn cloud_disable_clears_suspended_candidates_and_translation_review() {
    use crate::dispatch::{composed::Composed, session::SessionInfo, translate::Translation};
    use qingjian_core::{Candidate, CandidateKind, CandidateLayout};
    use qingjian_predict::PredictConfig;

    let dir = temp_dir("cloud-disable");
    let mut router = watched(&dir, "");
    let local = Candidate {
        text: "你".into(),
        kind: CandidateKind::Chinese,
        syllables: vec!["ni".into()],
        reading: None,
        translation: None,
        aux_code: None,
    };
    let composed = || {
        let mut layout = CandidateLayout::new(vec![local.clone()], 5, 1);
        let mut cloud = local.clone();
        cloud.text = "妮".into();
        cloud.kind = CandidateKind::Cloud;
        layout.set_cloud(vec![cloud]);
        Some(Composed::Candidates {
            preedit: Vec::new(),
            cursor: 0,
            layout,
            spelling: Default::default(),
            manual_sequence: None,
        })
    };
    router.composed = composed();
    let mut suspended = SessionInfo::new(None, false);
    suspended.composed = composed();
    router.sessions.insert(SessionId(2), suspended);
    router.translation = Some(Translation { result: None });
    router.pending_selection = Some((SessionId(1), 7));
    router.sentence = Some("旧服务补全".into());
    router.reload_cloud(&PredictConfig::default(), None);
    assert!(!router.engine.prediction_enabled());
    assert_eq!(router.config.cloud_slots, 0);
    assert!(router.translation.is_none());
    assert!(router.pending_selection.is_none());
    assert!(router.sentence.is_none());
    for composed in [&router.composed, &router.sessions[&SessionId(2)].composed] {
        let Some(Composed::Candidates { layout, .. }) = composed else {
            panic!("local candidates lost")
        };
        assert_eq!(layout.len(), 1);
        assert_eq!(layout.candidate(0), Some(&local));
    }
    std::fs::remove_dir_all(dir).unwrap();
}
