//! 「卡住时显示」的状态：这一段组句里有没有弹出过、上次敲到哪、停顿到什么时候算卡住（同 Windows Server 的那一份）。
//! Linux Server 不推送：到点靠前端组句时每 80 毫秒的 `Poll`（`Router::tick`）推进，弹出的那次回包才带上键盘。

use std::time::{Duration, Instant};

/// 停多久算卡住。熟练的人两键之间远短于这个，平时看不到键盘。与 macOS、Windows 一致。
const HESITATION: Duration = Duration::from_millis(700);

#[derive(Debug, Default)]
pub(crate) struct KeyHintState {
    /// 上次看到的缓冲区，变了就是又敲了一键，停顿重新计时。
    typed: String,

    /// 这一段组句里已经弹出过：之后一直留着到组句结束，不随每键闪。
    shown: bool,

    /// 停到这个时刻还没再敲就弹出。
    deadline: Option<Instant>,
}

impl KeyHintState {
    /// 这一帧要不要画键盘。`stuck` 是敲出了拼不成音节的键，当场弹出；否则缓冲区变了就重新计时。
    pub(crate) fn update(&mut self, typed: &str, stuck: bool) -> bool {
        if stuck || self.shown {
            self.shown = true;
            self.deadline = None;
            return true;
        }
        if typed != self.typed {
            typed.clone_into(&mut self.typed);
            self.deadline = Some(Instant::now() + HESITATION);
        }
        false
    }

    /// 停顿到点了就记成弹出过；没到点或没在计时什么都不做。
    pub(crate) fn advance(&mut self, now: Instant) {
        if self.deadline.is_some_and(|deadline| now >= deadline) {
            self.shown = true;
            self.deadline = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{HESITATION, KeyHintState};
    use std::time::{Duration, Instant};

    #[test]
    fn hesitation_waits_for_the_deadline() {
        let mut state = KeyHintState::default();
        assert!(!state.update("k", false));
        let deadline = state.deadline.unwrap();
        state.advance(deadline - Duration::from_nanos(1));
        assert!(!state.update("k", false));
        state.advance(deadline);
        assert!(state.update("k", false));
        assert!(state.update("ka", false));
    }

    #[test]
    fn changed_input_restarts_the_interval() {
        let mut state = KeyHintState::default();
        state.update("k", false);
        state.deadline = Some(Instant::now() - HESITATION);
        assert!(!state.update("ka", false));
        assert!(state.deadline.unwrap() > Instant::now());
    }
}
