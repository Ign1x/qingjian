//! 还没发给应用的文字更新：等应用落定再发（见 [`super::Session`] 的 `settle_at`）。

/// 合并起来的一次更新：上屏文字按先后拼在一起，拼音取最新的。
#[derive(Debug, Default)]
pub struct Unsent {
    pub commit: String,

    /// 有没发出去的更新（只换了拼音时 `commit` 是空的）。
    pub dirty: bool,
}

impl Unsent {
    pub fn push(&mut self, commit: &str) {
        self.commit.push_str(commit);
        self.dirty = true;
    }

    pub fn clear(&mut self) {
        self.commit.clear();
        self.dirty = false;
    }

    /// 取出要发的上屏文字；拼音由调用方用当前的那一份。
    pub fn take(&mut self) -> Option<String> {
        self.dirty.then(|| {
            self.dirty = false;
            std::mem::take(&mut self.commit)
        })
    }
}
