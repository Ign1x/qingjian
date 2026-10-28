//! 指定模型、下载后重新找文件、缺模型或运行库回退，以及旧用户目录兼容。
use super::{directory, touch};
use crate::LocalModel;
use crate::models::{ModelDirs, ModelFiles, find_model};

#[test]
fn requested_model_wins_and_missing_files_fall_back() {
    let root = directory("resolve");
    let dirs = ModelDirs {
        user: Some(root.join("user")),
        bundled_root: root.join("bundled"),
        runtime: Some(root.join("libonnxruntime.so")),
    };
    assert_eq!(dirs.resolve(LocalModel::Tongbian), None);
    let fallback = touch(
        &dirs.bundled_root,
        "data/models/hanzhang-tongbian/model.qjm",
    );
    for wanted in [LocalModel::Zhiyi, LocalModel::Zhiwei] {
        assert_eq!(
            dirs.resolve(wanted),
            Some(ModelFiles::Sentence(fallback.clone()))
        );
    }
    let user = dirs.user.as_ref().unwrap();
    let zhiwei = touch(user, "models/hanzhang-zhiwei/hanzhang-zhiwei-small.qjm");
    assert_eq!(
        dirs.resolve(LocalModel::Zhiwei),
        Some(ModelFiles::Sentence(zhiwei))
    );
    // 目录结构与设置页下载结果一致；现找文件，启动之后下载的也能选到。
    touch(user, "models/zhiyi-onnx/laya.int8g.onnx");
    assert_eq!(
        dirs.resolve(LocalModel::Zhiyi),
        Some(ModelFiles::Sentence(fallback.clone()))
    );
    let runtime = touch(&root, "libonnxruntime.so");
    assert_eq!(
        dirs.resolve(LocalModel::Zhiyi),
        Some(ModelFiles::Zhiyi {
            dir: user.join("models/zhiyi-onnx"),
            runtime,
        })
    );
    std::fs::remove_dir_all(user.join("models/zhiyi-onnx")).unwrap();
    assert_eq!(
        dirs.resolve(LocalModel::Zhiyi),
        Some(ModelFiles::Sentence(fallback))
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn p2c_precedes_old_model_and_user_precedes_bundled() {
    let root = std::env::temp_dir().join(format!("qingjian-model-choice-{}", std::process::id()));
    let user = root.join("user");
    let bundled = root.join("bundled");
    let _ = std::fs::remove_dir_all(&root);

    let old = bundled.join("data/models/hanzhang-zhiwei/model.qjm");
    std::fs::create_dir_all(old.parent().unwrap()).unwrap();
    std::fs::write(&old, b"").unwrap();
    assert_eq!(find_model(Some(&user), &bundled), Some(old));

    let user_old = user.join("models/hanzhang-zhiwei/model.qjm");
    std::fs::create_dir_all(user_old.parent().unwrap()).unwrap();
    std::fs::write(&user_old, b"").unwrap();
    assert_eq!(find_model(Some(&user), &bundled), Some(user_old));

    let bundled_p2c = bundled.join("data/models/hanzhang-tongbian/model.qjm");
    std::fs::create_dir_all(bundled_p2c.parent().unwrap()).unwrap();
    std::fs::write(&bundled_p2c, b"").unwrap();
    assert_eq!(find_model(Some(&user), &bundled), Some(bundled_p2c));

    let user_p2c = user.join("models/hanzhang-tongbian/model.qjm");
    std::fs::create_dir_all(user_p2c.parent().unwrap()).unwrap();
    std::fs::write(&user_p2c, b"").unwrap();
    assert_eq!(find_model(Some(&user), &bundled), Some(user_p2c.clone()));
    let legacy_p2c = user.join("model-p2c/model.qjm");
    std::fs::create_dir_all(legacy_p2c.parent().unwrap()).unwrap();
    std::fs::write(&legacy_p2c, b"").unwrap();
    assert_eq!(find_model(Some(&user), &bundled), Some(user_p2c.clone()));
    std::fs::remove_file(&user_p2c).unwrap();
    assert_eq!(find_model(Some(&user), &bundled), Some(legacy_p2c));
    std::fs::remove_dir_all(&root).unwrap();
}
