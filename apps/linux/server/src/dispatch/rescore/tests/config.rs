//! 模型开关、模型选择与知意档位的热加载。
use super::prefers::Prefers;
use super::{attach, compose, router};
use qingjian_platform::models::ModelDirs;
use qingjian_platform::{LocalModel, LocalModelConfig, ZhiyiTier};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

#[test]
fn disabling_or_changing_models_unloads_choice_and_stops_rescoring() {
    let mut router = router();
    let mut config = LocalModelConfig {
        name: LocalModel::Zhiyi,
        ..Default::default()
    };
    router.configure_local_model(ModelDirs::default(), &config);
    attach(&mut router, &Arc::new(Prefers::default()));
    compose(&mut router);
    config.enabled = false;
    router.apply_model_config(&config);
    assert!(!router.engine.has_choice_model());
    assert!(!router.engine.has_sentence_scorer());
    assert!(router.model_loader.is_none());
    assert_eq!(router.next_tick(), Duration::from_secs(1));

    config.enabled = true;
    router.apply_model_config(&config);
    attach(&mut router, &Arc::new(Prefers::default()));
    config.name = LocalModel::Zhiwei;
    router.apply_model_config(&config);
    assert!(!router.engine.has_choice_model());
}

#[test]
fn tier_change_keeps_the_model_and_changes_context_budget() {
    let mut router = router();
    let mut config = LocalModelConfig {
        name: LocalModel::Zhiyi,
        ..Default::default()
    };
    router.configure_local_model(ModelDirs::default(), &config);
    let model = Arc::new(Prefers::default());
    attach(&mut router, &model);
    for (tier, expected) in [(ZhiyiTier::Saver, 32), (ZhiyiTier::Accurate, 128)] {
        config.zhiyi_tier = tier;
        router.apply_model_config(&config);
        assert!(router.engine.has_choice_model());
        router.engine.clear();
        router.engine.set_rescoring_context(Some("字".repeat(150)));
        compose(&mut router);
        let deadline = Instant::now() + Duration::from_secs(3);
        while model.context_chars.load(Ordering::SeqCst) != expected {
            assert!(Instant::now() < deadline, "档位未应用");
            std::thread::sleep(Duration::from_millis(10));
            router.tick();
        }
    }
}

#[test]
fn disabling_while_loading_discards_the_receiver() {
    let root = std::env::temp_dir().join(format!("qj-model-cancel-{}", std::process::id()));
    let model_dir = root.join("models/hanzhang-zhiwei");
    std::fs::create_dir_all(&model_dir).unwrap();
    std::fs::write(model_dir.join("model.qjm"), b"bad model").unwrap();
    let mut router = router();
    let mut config = LocalModelConfig {
        name: LocalModel::Zhiwei,
        ..Default::default()
    };
    router.configure_local_model(
        ModelDirs {
            user: Some(root.clone()),
            ..Default::default()
        },
        &config,
    );
    assert!(router.model_loader.is_some());
    config.enabled = false;
    router.apply_model_config(&config);
    assert!(router.model_loader.is_none());
    router.tick();
    assert!(!router.engine.has_sentence_scorer());
    std::fs::remove_dir_all(root).unwrap();
}
