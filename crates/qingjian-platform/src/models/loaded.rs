//! 模型后台加载的轮询结果。

use qingjian_neural::NeuralError;

use super::LoadedModel;

pub enum Loaded {
    Pending,

    Done(Result<LoadedModel, NeuralError>),

    Gone,
}
