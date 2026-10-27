//! 被测进程退出时留下的。

#[derive(Debug)]
pub struct Exit {
    /// 退出码；被信号杀掉的是 `None`。
    pub code: Option<i32>,

    /// 进程打到 stderr 的全部（`--stderr` 下日志也在这里）。
    pub stderr: String,
}
