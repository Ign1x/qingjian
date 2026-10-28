//! 后台加载好的模型；两种用途分别接到 Engine 的整句与选词接口。

use std::sync::Arc;

use qingjian_core::ChoiceModel;
use qingjian_core::sentence::SentenceScorer;

pub enum LoadedModel {
    Sentence(Box<dyn SentenceScorer>),

    Choice(Arc<dyn ChoiceModel>),
}
