//! 池里的一块缓冲：位置与尺寸，以及读出它的画面。

use std::sync::Arc;

use super::{Picture, Pool};

pub struct Buffer {
    pool: Arc<Pool>,

    /// 在池里的起点（字节）。
    offset: i32,

    width: i32,

    height: i32,

    /// 一行的字节数。
    stride: i32,
}

impl Buffer {
    pub fn new(pool: Arc<Pool>, offset: i32, width: i32, height: i32, stride: i32) -> Self {
        Self {
            pool,
            offset,
            width,
            height,
            stride,
        }
    }

    /// 尺寸，以及有没有画上东西：全透明的像素四个字节都是 0（预乘），有一个字节不是 0 就算画了。
    pub fn picture(&self) -> Picture {
        Picture {
            width: self.width,
            height: self.height,
            painted: self.painted().unwrap_or(false),
        }
    }

    fn painted(&self) -> Option<bool> {
        let map = self.pool.map()?;
        let offset = usize::try_from(self.offset).ok()?;
        let stride = usize::try_from(self.stride).ok()?;
        let row = usize::try_from(self.width).ok()? * 4;
        let rows = usize::try_from(self.height).ok()?;
        let pixels = map.get(offset..offset + stride * rows)?;
        Some(
            pixels
                .chunks(stride)
                .any(|line| line[..row.min(line.len())].iter().any(|byte| *byte != 0)),
        )
    }
}
