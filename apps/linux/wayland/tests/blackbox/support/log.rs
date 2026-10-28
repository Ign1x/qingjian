//! 按到达顺序记下的一串事（合成器收到的请求、假 Server 收到的消息、被测进程的日志行），
//! 别的线程往里记，测试线程等某一条出现。

use std::fmt::Debug;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// 等一件事最多多久。只有出错时才会等满；机器忙（别的测试与编译并行）时被测进程起得慢，留足余量。
pub const PATIENCE: Duration = Duration::from_secs(30);

pub struct Log<T> {
    shared: Arc<(Mutex<Vec<T>>, Condvar)>,
}

impl<T> Clone for Log<T> {
    fn clone(&self) -> Self {
        Self {
            shared: self.shared.clone(),
        }
    }
}

impl<T> Default for Log<T> {
    fn default() -> Self {
        Self {
            shared: Arc::new((Mutex::new(Vec::new()), Condvar::new())),
        }
    }
}

impl<T: Clone + Debug> Log<T> {
    pub fn push(&self, entry: T) {
        let (entries, changed) = &*self.shared;
        lock(entries).push(entry);
        changed.notify_all();
    }

    pub fn len(&self) -> usize {
        lock(&self.shared.0).len()
    }

    pub fn entries(&self) -> Vec<T> {
        lock(&self.shared.0).clone()
    }

    /// 第 `mark` 条及之后的。
    pub fn since(&self, mark: usize) -> Vec<T> {
        lock(&self.shared.0).iter().skip(mark).cloned().collect()
    }

    /// 等第 `from` 条起出现满足 `found` 的一条，返回它的下标；`timeout` 内没有是 `None`。
    pub fn wait_for(
        &self,
        from: usize,
        timeout: Duration,
        found: impl Fn(&T) -> bool,
    ) -> Option<usize> {
        let deadline = Instant::now() + timeout;
        let (entries, changed) = &*self.shared;
        let mut guard = lock(entries);
        loop {
            if let Some(index) = guard.iter().skip(from).position(&found) {
                return Some(from + index);
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return None;
            }
            guard = changed
                .wait_timeout(guard, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    /// 同 [`Self::wait_for`]，等满 [`PATIENCE`] 还没有就带着这段记录 panic；返回那一条。
    pub fn wait(&self, from: usize, what: &str, found: impl Fn(&T) -> bool) -> T {
        match self.wait_for(from, PATIENCE, found) {
            Some(index) => lock(&self.shared.0)[index].clone(),
            None => panic!(
                "等了 {PATIENCE:?} 还没有{what}，第 {from} 条起的记录：{:#?}",
                self.since(from)
            ),
        }
    }

    /// 等记录整体满足 `done`（比如连上与断开的次数对上），等满 [`PATIENCE`] 还不满足就带着记录 panic。
    pub fn settle(&self, what: &str, done: impl Fn(&[T]) -> bool) {
        let deadline = Instant::now() + PATIENCE;
        let (entries, changed) = &*self.shared;
        let mut guard = lock(entries);
        while !done(&guard) {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                let entries = guard.clone();
                drop(guard);
                panic!("等了 {PATIENCE:?} 还没有{what}，记录：{entries:#?}");
            }
            guard = changed
                .wait_timeout(guard, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }
}

/// 记录的线程 panic 了也照样读：测试要的正是出错前记下的那些。
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
