//! 被吃掉的键自己重复：应用收不到这些键，按住退格删拼音的重复只能由输入法来做。

use std::time::{Duration, Instant};

use super::Key;

/// 合成器没给 `repeat_info` 时的缺省（与常见桌面一致）：600 毫秒后每秒 25 次。
const DEFAULT_RATE: i32 = 25;
const DEFAULT_DELAY: i32 = 600;

#[derive(Debug)]
pub struct Repeat {
    /// 每秒几次；0 是不重复。
    rate: i32,

    /// 按住多少毫秒开始重复。
    delay: i32,

    /// 正在重复的键与它按下的时刻。
    key: Option<(Key, Instant)>,

    /// 下一次该重复的时刻。
    next: Option<Instant>,

    /// 刚交出去的那一次本该在什么时候（下一次从它算起，节奏才不被处理时间拖慢）。
    last: Option<Instant>,
}

impl Default for Repeat {
    fn default() -> Self {
        Self {
            rate: DEFAULT_RATE,
            delay: DEFAULT_DELAY,
            key: None,
            next: None,
            last: None,
        }
    }
}

impl Repeat {
    /// 合成器给的重复设置。负数不合协议，当不重复。
    pub fn configure(&mut self, rate: i32, delay: i32) {
        self.rate = rate.max(0);
        self.delay = delay.max(0);
    }

    /// 一个吃掉的键按下了：键位表说它会重复、重复也没关，就排上第一次。
    pub fn start(&mut self, key: Key, now: Instant) {
        if !key.repeats || self.rate == 0 {
            self.stop();
            return;
        }
        self.key = Some((key, now));
        self.next = Some(now + Duration::from_millis(self.delay as u64));
    }

    pub fn stop(&mut self) {
        self.key = None;
        self.next = None;
        self.last = None;
    }

    /// 到点了就交出要重复的键；排下一次由调用方在回包之后调 [`Self::schedule`]，Server 慢的时候一次也只处理一个，不会越积越多。
    pub fn due(&mut self, now: Instant) -> Option<Key> {
        let next = self.next?;
        if now < next {
            return None;
        }
        self.next = None;
        self.last = Some(next);
        self.key.map(|(key, _)| key)
    }

    /// 排下一次重复：从上一次本该到点的时刻算起，处理时间不拖慢节奏；处理得比间隔还慢时下一次立刻到点。
    pub fn schedule(&mut self, now: Instant) {
        if self.key.is_some() && self.rate > 0 {
            let interval = Duration::from_millis(1000 / self.rate as u64);
            let from = self.last.unwrap_or(now);
            self.next = Some((from + interval).max(now));
        }
    }

    pub fn holds(&self, code: u32) -> bool {
        self.key.is_some_and(|(key, _)| key.code == code)
    }

    pub fn deadline(&self) -> Option<Instant> {
        self.next
    }

    /// 合成器时间轴上的「现在」：按下那一刻的时间戳加上过去的毫秒数，补发给应用的按键用它。
    pub fn time(&self, now: Instant) -> u32 {
        self.key.map_or(0, |(key, pressed)| {
            key.time
                .wrapping_add(now.duration_since(pressed).as_millis() as u32)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> Key {
        Key {
            code: 14,
            keysym: 0xff08,
            mask: 0,
            pressed: true,
            repeats: true,
            time: 100,
            epoch: 0,
        }
    }

    /// 600 毫秒后开始，之后每 40 毫秒一次；处理花掉的时间不算进间隔，处理太慢时下一次立刻到点。
    #[test]
    fn repeats_keep_their_cadence() {
        let mut repeat = Repeat::default();
        let start = Instant::now();
        repeat.start(key(), start);
        assert_eq!(repeat.due(start + Duration::from_millis(599)), None);
        let first = start + Duration::from_millis(600);
        assert!(repeat.due(first).is_some());
        repeat.schedule(first + Duration::from_millis(15));
        assert_eq!(repeat.deadline(), Some(first + Duration::from_millis(40)));

        let second = first + Duration::from_millis(40);
        assert!(repeat.due(second).is_some());
        let late = second + Duration::from_millis(70);
        repeat.schedule(late);
        assert_eq!(repeat.deadline(), Some(late));
        assert_eq!(repeat.time(first), 700, "补发的时间戳接着按下那一刻算");
    }

    /// 修饰键这类不重复的键、rate 为 0 都不排。
    #[test]
    fn some_keys_never_repeat() {
        let mut repeat = Repeat::default();
        let now = Instant::now();
        repeat.start(
            Key {
                repeats: false,
                ..key()
            },
            now,
        );
        assert_eq!(repeat.deadline(), None);
        repeat.configure(0, 600);
        repeat.start(key(), now);
        assert_eq!(repeat.deadline(), None);
    }
}
