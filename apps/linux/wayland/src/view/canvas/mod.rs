//! 贴在 popup 上的那一张：候选窗或中 / 英提示，右键菜单弹着时把菜单叠在候选上面。input-method-v2 只有一个 popup、位置由合成器定，
//! 菜单只能和候选窗画进同一张位图，popup 跟着变大。这里记下两块各在合成位图的哪儿：
//! 从 surface 坐标认出鼠标落在什么上、算出收鼠标的区域。只做算术，不碰 Wayland 对象。

mod part;

#[cfg(test)]
mod tests;

use std::borrow::Cow;
use std::fmt::Write;

use qingjian_render::{HitTarget, Rendered};

use part::Part;

use super::{Placement, paste};
use crate::session::Spot;

pub struct Canvas {
    /// 合成位图的像素宽高（裁边之前）。左上与候选窗的位图对齐，菜单伸出候选窗的右下时跟着变大。
    width: u32,

    height: u32,

    /// 合成位图放进缓冲的裁边与尺寸。按候选窗的阴影边裁，弹出菜单时候选窗在 popup 里不挪。
    pub placement: Placement,

    window: Part,

    /// 弹着的右键菜单。
    menu: Option<Part>,

    /// 画位图用的倍数（点 → 像素）。
    scale: f32,

    /// 中 / 英提示不收鼠标，整个 surface 穿透。
    interactive: bool,
}

impl Canvas {
    /// `menu` 是菜单的位图，与菜单卡片左上角在候选窗内容区里的位置（点）；`scale` 是 popup 的缩放。
    pub fn new(window: &Rendered, menu: Option<(&Rendered, (f32, f32))>, scale: f64) -> Self {
        let window_part = Part::new(window, (0, 0));
        let (mut width, mut height) = (window.pixmap.width(), window.pixmap.height());
        let menu = menu.map(|(menu, (x, y))| {
            let left = window_part.left + x * window.scale - menu.content_x as f32;
            let top = window_part.top + y * window.scale - menu.content_y as f32;
            let part = Part::new(menu, (left.round() as i32, top.round() as i32));
            width = width.max(reach(part.origin.0, menu.pixmap.width()));
            height = height.max(reach(part.origin.1, menu.pixmap.height()));
            part
        });
        Self {
            width,
            height,
            placement: Placement::new(width, height, window.content_x, window.content_y, scale),
            window: window_part,
            menu,
            scale: window.scale,
            interactive: true,
        }
    }

    /// 中 / 英提示沿用候选的裁边、缩放与贴图，鼠标完全穿透。
    pub fn hint(rendered: &Rendered, scale: f64) -> Self {
        Self {
            interactive: false,
            ..Self::new(rendered, None, scale)
        }
    }

    /// 合成位图的像素宽（每行 `width * 4` 字节）。
    pub fn width(&self) -> u32 {
        self.width
    }

    /// 合成位图（预乘 RGBA）：先铺候选窗，再把菜单叠上去；没有菜单时就是候选窗的位图本身。
    /// `window` 与 `menu` 要与建它时的是同一份。
    pub fn compose<'a>(&self, window: &'a Rendered, menu: Option<&Rendered>) -> Cow<'a, [u8]> {
        let (Some(part), Some(menu)) = (&self.menu, menu) else {
            return Cow::Borrowed(window.pixmap.data());
        };
        let mut pixels = vec![0; self.width as usize * self.height as usize * 4];
        let window = &window.pixmap;
        paste(
            window.data(),
            window.width(),
            (0, 0),
            &mut pixels,
            self.width,
        );
        let menu = &menu.pixmap;
        paste(
            menu.data(),
            menu.width(),
            part.origin,
            &mut pixels,
            self.width,
        );
        Cow::Owned(pixels)
    }

    /// surface 上一点（逻辑像素）落在什么上。菜单盖在候选窗上面，先看菜单；候选右侧的译词也算那个候选；
    /// 落在卡片上别处、卡片外（阴影、补的边）都是 `None`。
    pub fn spot(&self, x: f64, y: f64) -> Option<Spot> {
        if !self.interactive {
            return None;
        }
        let (x, y) = self.pixel(x, y);
        if let Some(menu) = &self.menu
            && let Some((x, y)) = menu.local(x, y)
        {
            return Some(match menu.target(x, y) {
                Some(HitTarget::MenuItem(item)) => Spot::MenuItem(item),
                _ => Spot::Menu,
            });
        }
        let (x, y) = self.window.local(x, y)?;
        match self.window.target(x, y)? {
            HitTarget::Candidate(index)
            | HitTarget::Translation {
                candidate: index, ..
            } => Some(Spot::Candidate(index)),
            HitTarget::MenuItem(_) => None,
        }
    }

    /// surface 上一点换成候选窗内容区里的位置（点）：右键菜单的卡片左上角摆在这里，缩放变了重画时也摆回这里。
    pub fn anchor(&self, x: f64, y: f64) -> (f32, f32) {
        let (x, y) = self.pixel(x, y);
        (
            (x - self.window.left) / self.scale,
            (y - self.window.top) / self.scale,
        )
    }

    /// 收鼠标的区域，surface 逻辑像素的矩形 `(x, y, 宽, 高)`：候选窗与菜单的卡片。
    /// 阴影与补的边不收，鼠标穿过去落到下面的应用里。
    pub fn input_region(&self) -> Vec<(i32, i32, i32, i32)> {
        if !self.interactive {
            return Vec::new();
        }
        std::iter::once(&self.window)
            .chain(&self.menu)
            .filter_map(|part| self.logical(part))
            .collect()
    }

    /// 能点的候选与菜单项各自的中心（surface 逻辑像素），一项一段：`candidate0@32,41 item1@60,90`。
    /// 记进调试日志，排查点不中时对照合成器给的坐标；黑盒测试也按它点。
    pub fn describe(&self) -> String {
        if !self.interactive {
            return String::new();
        }
        let window = self
            .window
            .centers()
            .filter_map(|(target, x, y)| match target {
                HitTarget::Candidate(index) => Some((format!("candidate{index}"), x, y)),
                HitTarget::Translation { .. } | HitTarget::MenuItem(_) => None,
            });
        let menu =
            self.menu
                .iter()
                .flat_map(Part::centers)
                .filter_map(|(target, x, y)| match target {
                    HitTarget::MenuItem(item) => Some((format!("item{item}"), x, y)),
                    HitTarget::Candidate(_) | HitTarget::Translation { .. } => None,
                });
        let mut text = String::new();
        for (name, x, y) in window.chain(menu) {
            let (x, y) = self.surface(x, y);
            let gap = if text.is_empty() { "" } else { " " };
            let _ = write!(text, "{gap}{name}@{},{}", x.round(), y.round());
        }
        text
    }

    /// surface 坐标（逻辑像素）→ 合成位图里的像素：viewport 把缓冲缩放到逻辑尺寸（没有 viewport 时按整数缩放，算法一样），
    /// 再加回裁掉的左上边。
    fn pixel(&self, x: f64, y: f64) -> (f32, f32) {
        let (ratio_x, ratio_y) = self.ratio();
        let placement = &self.placement;
        (
            (x * ratio_x) as f32 + placement.crop_x as f32,
            (y * ratio_y) as f32 + placement.crop_y as f32,
        )
    }

    /// [`Self::pixel`] 反过来。
    fn surface(&self, x: f32, y: f32) -> (f64, f64) {
        let (ratio_x, ratio_y) = self.ratio();
        let placement = &self.placement;
        (
            f64::from(x - placement.crop_x as f32) / ratio_x,
            f64::from(y - placement.crop_y as f32) / ratio_y,
        )
    }

    /// 缓冲像素 ÷ 逻辑像素。
    fn ratio(&self) -> (f64, f64) {
        let placement = &self.placement;
        (
            f64::from(placement.width) / f64::from(placement.logical_width.max(1)),
            f64::from(placement.height) / f64::from(placement.logical_height.max(1)),
        )
    }

    /// 一块的卡片在 surface 上的矩形，往外取整，截在 surface 里。
    fn logical(&self, part: &Part) -> Option<(i32, i32, i32, i32)> {
        let (left, top) = self.surface(part.left, part.top);
        let (right, bottom) = self.surface(part.left + part.width, part.top + part.height);
        let placement = &self.placement;
        let left = left.floor().max(0.0) as i32;
        let top = top.floor().max(0.0) as i32;
        let right = (right.ceil() as i32).min(placement.logical_width);
        let bottom = (bottom.ceil() as i32).min(placement.logical_height);
        (right > left && bottom > top).then_some((left, top, right - left, bottom - top))
    }
}

/// 从 `origin` 起摆一段 `size` 像素，伸到哪；整段都在负半边时是 0。
fn reach(origin: i32, size: u32) -> u32 {
    u32::try_from(i64::from(origin) + i64::from(size)).unwrap_or(0)
}
