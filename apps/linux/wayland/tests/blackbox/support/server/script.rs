//! 测试给假 Server 排的事：按下的答复、要交出的译文。测试线程往里放，假 Server 的线程按到达顺序取。

use std::collections::VecDeque;

use super::Answer;
use super::hook::Hook;

#[derive(Default)]
pub struct Script {
    /// 按下的答复，按先后用掉；没排到的按下放行、帧不变。
    pub presses: VecDeque<Answer>,

    /// 选区翻译等着的译文：下一次轮询时交出去（换上译文那一帧、不再 `translation_pending`）。
    pub translation: Option<String>,

    /// 下次答复之前同步发出合成器更新，确定地复现 Server 往返期间输入框变化。
    pub before_reply: Option<Hook>,
}
