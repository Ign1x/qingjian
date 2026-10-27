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
}

impl Default for Repeat {
    fn default() -> Self {
        Self {
            rate: DEFAULT_RATE,
            delay: DEFAULT_DELAY,
            key: None,
            next: None,
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
    }

    /// 到点了就交出要重复的键；排下一次由调用方在回包之后调 [`Self::schedule`]，免得 Server 慢的时候越积越多。
    pub fn due(&mut self, now: Instant) -> Option<Key> {
        let next = self.next?;
        if now < next {
            return None;
        }
        self.next = None;
        self.key.map(|(key, _)| key)
    }

    /// 排下一次重复。
    pub fn schedule(&mut self, now: Instant) {
        if self.key.is_some() && self.rate > 0 {
            self.next = Some(now + Duration::from_millis(1000 / self.rate as u64));
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
