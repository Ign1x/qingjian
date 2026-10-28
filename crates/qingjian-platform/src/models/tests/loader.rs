//! 坏模型在加载线程报错，轮询不会阻塞输入线程。
use super::{directory, touch};
use crate::models::{Loaded, ModelFiles, ModelLoader};
use std::time::{Duration, Instant};

#[test]
fn corrupt_sentence_model_reports_failure_without_panicking() {
    let root = directory("bad-model");
    let loader = ModelLoader::spawn(ModelFiles::Sentence(touch(&root, "bad.qjm"))).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match loader.poll() {
            Loaded::Pending => {
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(5));
            }
            Loaded::Done(Err(_)) => break,
            _ => panic!("坏模型必须返回加载错误"),
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}
