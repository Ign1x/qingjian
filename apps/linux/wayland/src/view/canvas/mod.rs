//! 贴在 popup 上的那一张：候选窗或中 / 英提示，键位提示亮着时旁边再摆一块键盘，右键菜单弹着时把菜单叠在候选上面。
//! input-method-v2 只有一个 popup、位置由合成器定，键盘与菜单只能和候选窗画进同一张位图，popup 跟着变大。
//! 这里记下几块各在合成位图的哪儿：从 surface 坐标认出鼠标落在什么上、算出收鼠标的区域（键盘不收）。
//! 只做算术，不碰 Wayland 对象。

mod part;

#[cfg(test)]
mod tests;

use std::borrow::Cow;
use std::fmt::Write;

use qingjian_render::{HitTarget, Rendered};

use part::Part;

use super::{Placement, Side, paste, place};
use crate::session::Spot;

/// 键盘与候选窗之间的间隙（逻辑像素）：乘上缩放就是位图里的像素。
const GAP_POINTS: f32 = 4.0;

pub struct Canvas {
    /// 合成位图的像素宽高（裁边之前）。左上与候选窗的位图对齐，菜单、键盘伸出候选窗时跟着变大。
    width: u32,

    height: u32,

    /// 合成位图放进缓冲的裁边与尺寸。按各块内容区的左上角取能裁掉的透明边，弹出菜单、键盘时候选窗在 popup 里不挪。
    pub placement: Placement,

    window: Part,

    /// 弹着的右键菜单。
    menu: Option<Part>,

    /// 画着的键位提示键盘；它不收鼠标，只占画面。
    keyboard: Option<Part>,

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
            keyboard: None,
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

    /// 把画好的键盘摆进合成位图，返回这段输入用的那一侧（下一次先试它，键盘不随光标左右跳）。
    /// `flipped` 是合成器把 popup 翻到光标上方。键盘位图的原点按内容位置减位图阴影边摆，负的那一截贴图时裁掉。
    pub fn place_key_hint(
        &mut self,
        keyboard: &Rendered,
        flipped: bool,
        sticky: Option<Side>,
    ) -> Side {
        let gap = (GAP_POINTS * self.scale).round() as i32;
        let candidate = (
            self.window.left as i32,
            self.window.top as i32,
            self.window.width as u32,
            self.window.height as u32,
        );
        let (side, shift, content) = place(
            candidate,
            (
                keyboard.content_width,
                keyboard.content_height,
                keyboard.content_x,
                keyboard.content_y,
            ),
            flipped,
            sticky,
            gap,
        );
        // 候选窗与菜单一起挪（菜单相对候选窗的位置不变）
        self.window.shift(shift.0, shift.1);
        if let Some(menu) = &mut self.menu {
            menu.shift(shift.0, shift.1);
        }
        let keyboard = Part::new(
            keyboard,
            (
                content.0 - keyboard.content_x as i32,
                content.1 - keyboard.content_y as i32,
            ),
        );
        let mut width = reach(self.window.origin.0, self.window.size.0)
            .max(reach(keyboard.origin.0, keyboard.size.0));
        let mut height = reach(self.window.origin.1, self.window.size.1)
            .max(reach(keyboard.origin.1, keyboard.size.1));
        let mut margin_x = self.window.left.min(keyboard.left);
        let mut margin_y = self.window.top.min(keyboard.top);
        if let Some(menu) = &self.menu {
            width = width.max(reach(menu.origin.0, menu.size.0));
            height = height.max(reach(menu.origin.1, menu.size.1));
            margin_x = margin_x.min(menu.left);
            margin_y = margin_y.min(menu.top);
        }
        self.width = width;
        self.height = height;
        // 各块内容区起点里最靠左 / 最靠上的那个，就是位图左上能整个裁掉的透明边
        self.placement = Placement::new(
            width,
            height,
            margin_x.max(0.0).round() as u32,
            margin_y.max(0.0).round() as u32,
            f64::from(self.scale),
        );
        self.keyboard = Some(keyboard);
        side
    }

    /// 合成位图的像素宽（每行 `width * 4` 字节）。
    pub fn width(&self) -> u32 {
        self.width
    }

    /// 合成位图（预乘 RGBA）：先铺键盘，再铺候选窗，最后把菜单叠上去；没有菜单也没有键盘时就是候选窗的位图本身。
    /// `window`、`menu`、`keyboard` 要与建它并摆好位置时的是同一份。
    pub fn compose<'a>(
        &self,
        window: &'a Rendered,
        menu: Option<&Rendered>,
        keyboard: Option<&Rendered>,
    ) -> Cow<'a, [u8]> {
        let menu = self.menu.as_ref().zip(menu);
        let keyboard = self.keyboard.as_ref().zip(keyboard);
        if menu.is_none() && keyboard.is_none() {
            return Cow::Borrowed(window.pixmap.data());
        }
        let mut pixels = vec![0; self.width as usize * self.height as usize * 4];
        let mut draw = |part: &Part, rendered: &Rendered| {
            let pixmap = &rendered.pixmap;
            paste(
                pixmap.data(),
                pixmap.width(),
                part.origin,
                &mut pixels,
                self.width,
            );
        };
        if let Some((part, rendered)) = keyboard {
            draw(part, rendered);
        }
        draw(&self.window, window);
        if let Some((part, rendered)) = menu {
            draw(part, rendered);
        }
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
