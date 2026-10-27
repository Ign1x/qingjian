//! 按着的键各自按下那次是怎么处置的：抬起跟着按下走。

mod held;

pub use held::Held;

/// 按着的键。几根手指的事，线性找就够。
#[derive(Debug, Default)]
pub struct Keys {
    held: Vec<(u32, Held)>,
}

impl Keys {
    /// 记下一次按下；同一个键再按一次（重复）覆盖前一次。
    pub fn press(&mut self, code: u32, held: Held) {
        match self.held.iter_mut().find(|(key, _)| *key == code) {
            Some((_, entry)) => *entry = held,
            None => self.held.push((code, held)),
        }
    }

    /// 抬起：交回按下那次的处置；没见过它的按下（grab 之前就按着的）是 `None`。
    pub fn release(&mut self, code: u32) -> Option<Held> {
        let index = self.held.iter().position(|(key, _)| *key == code)?;
        Some(self.held.swap_remove(index).1)
    }

    /// 改成已转发：吃掉的键在重复途中被 Server 放行，之后的抬起也要转给应用。
    pub fn forward(&mut self, code: u32) {
        if let Some((_, held)) = self.held.iter_mut().find(|(key, _)| *key == code) {
            held.forwarded = true;
        }
    }

    pub fn holds(&self, code: u32) -> bool {
        self.held.iter().any(|(key, _)| *key == code)
    }

    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    /// 转发过、还没抬起的键：退出前要替它们补抬起，不然虚拟键盘上一直按着。
    pub fn drain_forwarded(&mut self) -> Vec<u32> {
        let codes = self
            .held
            .iter()
            .filter(|(_, held)| held.forwarded)
            .map(|(code, _)| *code)
            .collect();
        self.held.clear();
        codes
    }
}
