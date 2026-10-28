//! 滚轮与触摸板的滚动折成翻页：滚轮一格一页，高精度滚轮按 120 分之一格累计，触摸板这类连续滚动按距离累计。
//! 一个 `frame` 里的几样（`axis_value120` 或 `axis_discrete`，加上 `axis` 的距离）说的是同一下，只算一次；
//! 一个 `frame` 最多翻一页，多出来的格数与距离不留到下一次，别一下滚出好几页。只看竖直方向。

/// 滚轮一格在 `axis_value120` 里是 120。
const NOTCH: i32 = 120;

/// 触摸板这类连续滚动滚多远（逻辑像素）翻一页。
const SWIPE: f64 = 50.0;

#[derive(Debug, Default)]
pub struct Wheel {
    /// 这一帧报的格数（120 分之一格）；没报是 `None`，那就按 `axis` 的距离算。
    value120: Option<i32>,

    /// 这一帧 `axis` 报的距离。
    distance: f64,

    /// 跨帧攒着、还不够一格的格数。
    notches: i32,

    /// 跨帧攒着、还不够一页的距离；手指离开触摸板（`axis_stop`）时清掉。
    swiped: f64,
}

impl Wheel {
    pub fn axis(&mut self, value: f64) {
        self.distance += value;
    }

    /// wl_pointer 第 5–7 版的格数（第 8 版起换成 [`Self::value120`]）。
    pub fn discrete(&mut self, steps: i32) {
        self.value120(steps.saturating_mul(NOTCH));
    }

    pub fn value120(&mut self, value: i32) {
        let notches = self.value120.get_or_insert(0);
        *notches = notches.saturating_add(value);
    }

    /// 手指离开了触摸板：攒着的距离清掉，下一次滑动从头算。
    pub fn stop(&mut self) {
        self.swiped = 0.0;
    }

    /// 一帧结束：够一页就交出方向，`true` 是往下（下一页）。
    pub fn frame(&mut self) -> Option<bool> {
        let distance = std::mem::take(&mut self.distance);
        if let Some(value120) = self.value120.take() {
            // 有格数就按格算，`axis` 的距离说的是同一下
            let total = self.notches.saturating_add(value120);
            if total.abs() < NOTCH {
                self.notches = total;
                return None;
            }
            self.notches = 0;
            return Some(total > 0);
        }
        if distance == 0.0 {
            return None;
        }
        // 换了方向从头攒，不然先得把反方向攒的那一截滚回来
        if self.swiped * distance < 0.0 {
            self.swiped = 0.0;
        }
        self.swiped += distance;
        if self.swiped.abs() < SWIPE {
            return None;
        }
        let next = self.swiped > 0.0;
        self.swiped = 0.0;
        Some(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 一格滚轮：第 8 版的 value120 与同一下的 axis 距离一起来，只翻一页；第 5–7 版的 discrete 也一样。
    #[test]
    fn a_notch_turns_one_page() {
        let mut wheel = Wheel::default();
        wheel.value120(120);
        wheel.axis(15.0);
        assert_eq!(wheel.frame(), Some(true));
        wheel.discrete(-1);
        wheel.axis(-15.0);
        assert_eq!(wheel.frame(), Some(false));
        assert_eq!(wheel.frame(), None, "空帧不翻");
    }

    /// 高精度滚轮四分之一格一报：攒够一格翻一页，不够的留着；一帧里滚了三格也只翻一页，多的不留。
    #[test]
    fn fine_notches_add_up_but_never_flip_twice() {
        let mut wheel = Wheel::default();
        for _ in 0..3 {
            wheel.value120(30);
            assert_eq!(wheel.frame(), None);
        }
        wheel.value120(30);
        assert_eq!(wheel.frame(), Some(true));
        wheel.value120(360);
        assert_eq!(wheel.frame(), Some(true));
        wheel.value120(30);
        assert_eq!(wheel.frame(), None, "上一帧多出来的两格不留");
        wheel.value120(-150);
        assert_eq!(
            wheel.frame(),
            Some(false),
            "攒着的 30 与反方向的 150 相抵，还够一格"
        );
    }

    /// 触摸板：按距离攒够一页翻一页，翻完从头攒；手指离开就清掉；换了方向从头攒。
    #[test]
    fn swipes_add_up_by_distance() {
        let mut wheel = Wheel::default();
        for _ in 0..4 {
            wheel.axis(12.0);
            assert_eq!(wheel.frame(), None);
        }
        wheel.axis(12.0);
        assert_eq!(wheel.frame(), Some(true));
        wheel.axis(200.0);
        assert_eq!(wheel.frame(), Some(true), "一帧滑得再远也只翻一页");
        wheel.axis(40.0);
        assert_eq!(wheel.frame(), None, "翻完从头攒");

        wheel.stop();
        wheel.axis(40.0);
        assert_eq!(wheel.frame(), None, "抬过手，前一次滑的不算");
        wheel.axis(-45.0);
        assert_eq!(wheel.frame(), None, "换了方向从头攒");
        wheel.axis(-10.0);
        assert_eq!(wheel.frame(), Some(false));
    }
}
