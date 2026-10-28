//! 模型的后台加载：加载并预热要几百毫秒到几秒，不能挡住按键；加载完由 Router 在下一次消息时接上。

use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use qingjian_neural::zhiyi::ChoiceScorer;
use qingjian_neural::{CharScorer, NeuralError, P2cScorer};

use super::{Loaded, LoadedModel, ModelFiles};

/// 知意用几个线程：多了发热，两个线程准确档一次也只要 30–40 ms。
const ZHIYI_THREADS: usize = 2;

/// 一次进行中的加载。
pub struct ModelLoader {
    result: Receiver<Result<LoadedModel, NeuralError>>,
}

impl ModelLoader {
    /// 起线程加载并预热一次；线程起不来返回 `None`。
    pub fn spawn(files: ModelFiles) -> Option<Self> {
        let (tx, result) = channel();
        let spawned = std::thread::Builder::new()
            .name("qingjian-model-load".to_owned())
            .spawn(move || {
                let started = std::time::Instant::now();
                let loaded = match &files {
                    ModelFiles::Sentence(path) => load_sentence(path),
                    ModelFiles::Zhiyi { dir, runtime } => load_zhiyi(dir, runtime),
                };
                if let Ok((_, kind)) = &loaded {
                    let path = match &files {
                        ModelFiles::Sentence(path) => path,
                        ModelFiles::Zhiyi { dir, .. } => dir,
                    };
                    tracing::info!(
                        path = %path.display(),
                        kind,
                        total_ms = started.elapsed().as_millis(),
                        "本地模型已加载并预热"
                    );
                }
                let _ = tx.send(loaded.map(|(model, _)| model));
            });
        match spawned {
            Ok(_) => Some(Self { result }),
            Err(error) => {
                tracing::warn!(%error, "起不了模型加载线程，本地模型不用");
                None
            }
        }
    }

    /// 看一眼有没有结果，不阻塞。
    pub fn poll(&self) -> Loaded {
        match self.result.try_recv() {
            Ok(result) => Loaded::Done(result),
            Err(TryRecvError::Empty) => Loaded::Pending,
            Err(TryRecvError::Disconnected) => Loaded::Gone,
        }
    }
}

fn load_sentence(path: &std::path::Path) -> Result<(LoadedModel, &'static str), NeuralError> {
    let scorer = CharScorer::load(path)?;
    if scorer.vocab().sep().is_some() {
        let p2c = P2cScorer::new(scorer).expect("P2C vocab checked above");
        p2c.0.score_p2c("ni", &["你"])?;
        Ok((LoadedModel::Sentence(Box::new(p2c)), "含章·通变"))
    } else {
        scorer.score("", &["的"])?;
        Ok((LoadedModel::Sentence(Box::new(scorer)), "含章·知微"))
    }
}

/// 预热一次：第一次前向要做图优化。
fn load_zhiyi(
    dir: &std::path::Path,
    runtime: &std::path::Path,
) -> Result<(LoadedModel, &'static str), NeuralError> {
    let scorer = ChoiceScorer::load_onnx(dir, runtime, ZHIYI_THREADS)?;
    let typed = scorer.letter_units("ni");
    scorer.score("", &typed, &["ni", "nin"])?;
    Ok((LoadedModel::Choice(Arc::new(scorer)), "知意"))
}
