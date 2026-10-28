//! 用内存通道控制云请求和响应，不连接网络。

use qingjian_core::{Prediction, PredictionPolicy, PredictionRequest, Predictor};
use std::sync::mpsc::{Receiver, Sender};

pub(super) struct ChannelPredictor {
    /// 是否支持停键后自动联想；`false` 模拟关掉「自动联想」只留按需触发。
    pub(super) automatic: bool,

    pub(super) requests: Sender<PredictionRequest>,

    pub(super) responses: Receiver<Prediction>,
}

impl Predictor for ChannelPredictor {
    fn policy(&self) -> PredictionPolicy {
        PredictionPolicy {
            automatic: self.automatic,
            slots: 2,
            ..PredictionPolicy::default()
        }
    }

    fn submit(&mut self, request: PredictionRequest) {
        self.requests.send(request).unwrap();
    }

    fn poll(&mut self) -> Option<Prediction> {
        self.responses.try_recv().ok()
    }
}
