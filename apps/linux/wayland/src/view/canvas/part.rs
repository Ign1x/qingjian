//! 合成位图里的一块：候选窗或右键菜单的位图摆在哪、内容区（卡片，不含阴影）在哪，以及它能点的区域。

use qingjian_render::{HitRegion, HitTarget, Rendered};

pub struct Part {
    /// 位图左上角在合成位图里的像素位置；菜单的可以是负的（左上的阴影出了界，那一截不画）。
    pub origin: (i32, i32),

    /// 内容区左上角在合成位图里的像素位置。
    pub left: f32,

    pub top: f32,

    /// 内容区的像素宽高。
    pub width: f32,

    pub height: f32,

    /// 能点的区域，内容区像素（`Rendered::hits`）。
    hits: Vec<HitRegion>,
}

impl Part {
    pub fn new(rendered: &Rendered, origin: (i32, i32)) -> Self {
        Self {
            origin,
            left: origin.0 as f32 + rendered.content_x as f32,
            top: origin.1 as f32 + rendered.content_y as f32,
            width: rendered.content_width as f32,
            height: rendered.content_height as f32,
            hits: rendered.hits.clone(),
        }
    }

    /// 合成位图里一点（像素）在内容区里时，交出它在内容区里的位置；卡片外（阴影、补的边）是 `None`。
    pub fn local(&self, x: f32, y: f32) -> Option<(f32, f32)> {
        let (x, y) = (x - self.left, y - self.top);
        (x >= 0.0 && y >= 0.0 && x < self.width && y < self.height).then_some((x, y))
    }

    /// 内容区里这一点点中的目标；译词排在前面，先命中。
    pub fn target(&self, x: f32, y: f32) -> Option<HitTarget> {
        self.hits
            .iter()
            .find(|region| region.contains(x, y))
            .map(|region| region.target)
    }

    /// 能点的区域各自的中心与目标（合成位图里的像素）。
    pub fn centers(&self) -> impl Iterator<Item = (HitTarget, f32, f32)> + '_ {
        self.hits.iter().map(|region| {
            (
                region.target,
                self.left + region.x + region.width / 2.0,
                self.top + region.y + region.height / 2.0,
            )
        })
    }
}
