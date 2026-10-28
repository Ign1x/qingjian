//! 位图怎么放进 popup 的 shm 缓冲：裁掉左上多余的阴影边，按缩放定逻辑尺寸与缓冲尺寸，再换成 Wayland 的像素格式；
//! 右键菜单弹着时先把它叠到候选窗的位图上（[`paste`]）。
//! 只做算术，不碰 Wayland 对象；规则见 `docs/design/linux-wayland.md`「显示」的缩放与像素格式两条。

/// 左上的阴影边留几点：合成器把 popup 左上角贴在光标上，阴影边整个留着候选框就离光标太远。
const KEEP_POINTS: f64 = 4.0;

/// 像素 ÷ 缩放本该整除时，浮点误差可能让商多出一点点，向上取整就平白多一个逻辑像素。
const EPSILON: f64 = 1e-9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    /// 从位图左边裁掉的像素（阴影边）。
    pub crop_x: u32,

    /// 从位图上边裁掉的像素。
    pub crop_y: u32,

    /// 缓冲宽高（像素）：不小于裁剩的位图，右侧与下方透明补齐。
    pub width: u32,

    pub height: u32,

    /// surface 的逻辑尺寸，即 `wp_viewport` 的目标尺寸。
    pub logical_width: i32,

    pub logical_height: i32,
}

impl Placement {
    /// `width` / `height` 是位图的像素宽高，`content_x` / `content_y` 是内容区在位图里的位置
    /// （`Rendered` 的同名字段，即左上阴影边）；`scale` 是画位图用的倍数。
    pub fn new(width: u32, height: u32, content_x: u32, content_y: u32, scale: f64) -> Self {
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        let keep = (KEEP_POINTS * scale).round() as u32;
        let (crop_x, width, logical_width) = axis(width, content_x, keep, scale);
        let (crop_y, height, logical_height) = axis(height, content_y, keep, scale);
        Self {
            crop_x,
            crop_y,
            width,
            height,
            logical_width,
            logical_height,
        }
    }
}

/// 一个方向上的（裁掉的像素，缓冲像素，逻辑尺寸）。逻辑尺寸取 `ceil(可见 ÷ 缩放)`，缓冲取 `round(逻辑 × 缩放)`：
/// 与分数缩放协议给的换算一致，合成器按 1:1 贴、不糊；整数缩放时缓冲正好是逻辑尺寸的整数倍。
fn axis(size: u32, content: u32, keep: u32, scale: f64) -> (u32, u32, i32) {
    let crop = content.saturating_sub(keep).min(size.saturating_sub(1));
    let visible = (size - crop).max(1);
    let logical = ((f64::from(visible) / scale - EPSILON).ceil() as i32).max(1);
    let buffer = ((f64::from(logical) * scale).round() as u32).max(visible);
    (crop, buffer, logical)
}

/// 预乘 RGBA 位图（每行 `src_width * 4` 字节）→ Wayland `ARGB8888`（小端，内存序 B,G,R,A，同样预乘），
/// 写进每行 `placement.width * 4` 字节、共 `placement.height` 行的 `dst`。裁掉的左上部分跳过，补边填 0。
pub fn blit(pixels: &[u8], src_width: u32, placement: &Placement, dst: &mut [u8]) {
    debug_assert_eq!(
        dst.len(),
        placement.width as usize * placement.height as usize * 4
    );
    let src_stride = src_width as usize * 4;
    let dst_stride = placement.width as usize * 4;
    if dst_stride == 0 {
        return;
    }
    let skip = (placement.crop_x as usize * 4).min(src_stride);
    let mut rows = (src_stride > 0)
        .then(|| pixels.chunks_exact(src_stride))
        .into_iter()
        .flatten()
        .skip(placement.crop_y as usize)
        .map(|row| &row[skip..]);
    for out in dst.chunks_mut(dst_stride) {
        let row = rows.next().unwrap_or_default();
        let copied = out.len().min(row.len()) / 4 * 4;
        for (to, from) in out[..copied].chunks_exact_mut(4).zip(row.chunks_exact(4)) {
            to.copy_from_slice(&[from[2], from[1], from[0], from[3]]);
        }
        out[copied..].fill(0);
    }
}

/// 把一张预乘 RGBA 位图（每行 `src_width * 4` 字节）按 source-over 叠到另一张上（每行 `dst_width * 4` 字节），
/// 左上角摆在 `at`（像素，可以是负的）；出了 `dst` 的部分不画。
pub fn paste(src: &[u8], src_width: u32, at: (i32, i32), dst: &mut [u8], dst_width: u32) {
    let (src_stride, dst_stride) = (src_width as usize * 4, dst_width as usize * 4);
    if src_stride == 0 || dst_stride == 0 {
        return;
    }
    let src_height = (src.len() / src_stride) as i64;
    let dst_height = (dst.len() / dst_stride) as i64;
    let (at_x, at_y) = (i64::from(at.0), i64::from(at.1));
    // 两张在 x 方向上重叠的那一段，按源位图的列算
    let first = (-at_x).max(0);
    let last = i64::from(src_width).min(i64::from(dst_width) - at_x);
    if first >= last {
        return;
    }
    let columns = (last - first) as usize * 4;
    for row in (-at_y).max(0)..src_height.min(dst_height - at_y) {
        let from = row as usize * src_stride + first as usize * 4;
        let to = (row + at_y) as usize * dst_stride + (first + at_x) as usize * 4;
        let pixels = src[from..from + columns].chunks_exact(4);
        for (under, over) in dst[to..to + columns].chunks_exact_mut(4).zip(pixels) {
            let keep = 255 - u32::from(over[3]);
            for (channel, top) in under.iter_mut().zip(over) {
                let blended = u32::from(*top) + (u32::from(*channel) * keep + 127) / 255;
                *channel = blended.min(255) as u8;
            }
        }
    }
}
