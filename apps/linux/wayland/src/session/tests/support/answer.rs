//! 会话测试的脚本答复。

use qingjian_platform::protocol::{Frame, KeyOutcome};

/// 一次按下的脚本化答复。
pub struct Answer {
    pub outcome: KeyOutcome,

    pub commit: Option<String>,

    pub frame: Frame,

    pub english: Option<bool>,
}
