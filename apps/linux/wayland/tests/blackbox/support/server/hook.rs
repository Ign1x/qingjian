//! 在指定消息的回包之前插入合成器更新，后台轮询不会提前触发它。

pub struct Hook {
    pub kind: &'static str,

    pub action: Box<dyn FnOnce() + Send>,
}
