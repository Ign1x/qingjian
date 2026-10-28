//! 托盘图标：中、英两张自己画的（白字压在品牌绿的圆角方块上，深浅两种状态栏上都看得清），
//! 还不知道模式时用青简标志（与 Fcitx5 托盘里的一样）。各备几种尺寸，托盘宿主按自己要的大小挑。

use bamboo_slip::{Pixmap, SvgImage};
use qingjian_platform::protocol::linux::InputMode;

/// 一张图：宽、高、ARGB32 像素（网络字节序，不预乘），即 StatusNotifierItem 的 `a(iiay)` 里的一项。
pub type Picture = (i32, i32, Vec<u8>);

/// 备的尺寸：常见托盘 16–24 像素，高分屏上翻倍。
const SIZES: [u32; 6] = [16, 22, 24, 32, 48, 64];

const CHINESE: &[u8] = include_bytes!("../../../../../assets/icon/linux/tray-zh.svg");

const ENGLISH: &[u8] = include_bytes!("../../../../../assets/icon/linux/tray-en.svg");

/// 青简标志，与装进 hicolor 的那几张同一份（安装脚本把它们装成 `qingjian` 图标）。
const LOGO: [&[u8]; 6] = [
    include_bytes!("../../../../../assets/icon/linux/hicolor/16x16/apps/qingjian.png"),
    include_bytes!("../../../../../assets/icon/linux/hicolor/22x22/apps/qingjian.png"),
    include_bytes!("../../../../../assets/icon/linux/hicolor/24x24/apps/qingjian.png"),
    include_bytes!("../../../../../assets/icon/linux/hicolor/32x32/apps/qingjian.png"),
    include_bytes!("../../../../../assets/icon/linux/hicolor/48x48/apps/qingjian.png"),
    include_bytes!("../../../../../assets/icon/linux/hicolor/64x64/apps/qingjian.png"),
];

pub struct Icons {
    chinese: Vec<Picture>,

    english: Vec<Picture>,

    logo: Vec<Picture>,
}

impl Icons {
    /// 启动时栅格一遍（几毫秒）。随包的图坏了只会是打包出错，那一种就是空的（托盘宿主显示缺图标）。
    pub fn new() -> Self {
        Self {
            chinese: rasterize(CHINESE),
            english: rasterize(ENGLISH),
            logo: LOGO
                .iter()
                .filter_map(|png| Pixmap::decode_png(png).ok())
                .map(|pixmap| picture(&pixmap))
                .collect(),
        }
    }

    /// 这个模式的图标；不知道模式时是青简标志。
    pub fn get(&self, mode: Option<&InputMode>) -> &[Picture] {
        match mode {
            None => &self.logo,
            Some(mode) if mode.english => &self.english,
            Some(_) => &self.chinese,
        }
    }
}

/// 一张 SVG 栅格成 [`SIZES`] 那几种尺寸。
fn rasterize(svg: &[u8]) -> Vec<Picture> {
    let Ok(image) = SvgImage::parse(svg) else {
        tracing::warn!("托盘图标的 SVG 解不开");
        return Vec::new();
    };
    SIZES
        .iter()
        .filter_map(|&size| image.raster(size, size))
        .map(|pixmap| picture(&pixmap))
        .collect()
}

/// 预乘 RGBA 的位图 → StatusNotifierItem 要的 ARGB32：每像素 A、R、G、B 四个字节（网络字节序），颜色不预乘。
pub fn picture(pixmap: &Pixmap) -> Picture {
    let pixels = pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let color = pixel.demultiply();
            [color.alpha(), color.red(), color.green(), color.blue()]
        })
        .collect();
    (pixmap.width() as i32, pixmap.height() as i32, pixels)
}
