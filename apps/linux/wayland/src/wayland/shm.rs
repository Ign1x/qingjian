//! 共享内存缓冲：几块轮换，合成器还没还回来的不碰。

use std::fs::File;
use std::os::fd::AsFd;

use memmap2::MmapMut;
use wayland_client::protocol::wl_buffer::{self, WlBuffer};
use wayland_client::protocol::wl_shm::{Format, WlShm};
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::{Connection, Dispatch, QueueHandle};

use super::{Wayland, memfd};

/// 最多几块：一块在合成器手里显示、一块在画，动画时偶尔还要第三块。
const MAX_SLOTS: usize = 3;

#[derive(Default)]
pub struct Buffers {
    slots: Vec<Slot>,
}

/// 一块缓冲：匿名文件、它的映射、池，以及按当前尺寸建好的 wl_buffer。
struct Slot {
    file: File,

    map: MmapMut,

    pool: WlShmPool,

    /// 池的字节数，只增不减。
    size: usize,

    /// 建好的 wl_buffer 与它的宽高。
    buffer: Option<(WlBuffer, u32, u32)>,

    /// 交给了合成器、还没收到 release。
    busy: bool,
}

impl Buffers {
    /// 拿一块空闲的、正好 `width`×`height` 的缓冲：可写的像素与 wl_buffer。都在合成器手里时返回 `None`。
    pub fn take(
        &mut self,
        shm: &WlShm,
        qh: &QueueHandle<Wayland>,
        width: u32,
        height: u32,
    ) -> Option<(&mut [u8], WlBuffer)> {
        let len = width as usize * height as usize * 4;
        let index = match self.slots.iter().position(|slot| !slot.busy) {
            Some(index) => index,
            None if self.slots.len() < MAX_SLOTS => {
                let slot = Slot::new(shm, qh, len)
                    .inspect_err(|error| tracing::warn!(%error, "建共享内存缓冲失败"))
                    .ok()?;
                self.slots.push(slot);
                self.slots.len() - 1
            }
            None => return None,
        };
        let slot = &mut self.slots[index];
        if slot.size < len
            && let Err(error) = slot.grow(len)
        {
            tracing::warn!(%error, "共享内存缓冲扩容失败");
            return None;
        }
        if slot
            .buffer
            .as_ref()
            .is_none_or(|(_, w, h)| (*w, *h) != (width, height))
        {
            if let Some((old, _, _)) = slot.buffer.take() {
                old.destroy();
            }
            let buffer = slot.pool.create_buffer(
                0,
                width as i32,
                height as i32,
                width as i32 * 4,
                Format::Argb8888,
                qh,
                index,
            );
            slot.buffer = Some((buffer, width, height));
        }
        slot.busy = true;
        let buffer = slot.buffer.as_ref().map(|(buffer, _, _)| buffer.clone())?;
        Some((&mut slot.map[..len], buffer))
    }
}

impl Slot {
    fn new(shm: &WlShm, qh: &QueueHandle<Wayland>, size: usize) -> std::io::Result<Self> {
        let size = size.max(1);
        let file = memfd::create(c"qingjian-popup", size)?;
        // SAFETY: 文件是我们自己的匿名文件，长度刚设好
        let map = unsafe { MmapMut::map_mut(&file)? };
        let pool = shm.create_pool(file.as_fd(), size as i32, qh, ());
        Ok(Self {
            file,
            map,
            pool,
            size,
            buffer: None,
            busy: false,
        })
    }

    /// 池只能变大：文件加长、重新映射、通知合成器。
    fn grow(&mut self, size: usize) -> std::io::Result<()> {
        self.file.set_len(size as u64)?;
        // SAFETY: 同上
        self.map = unsafe { MmapMut::map_mut(&self.file)? };
        self.pool.resize(size as i32);
        self.size = size;
        Ok(())
    }
}

impl Dispatch<WlBuffer, usize> for Wayland {
    fn event(
        state: &mut Self,
        _: &WlBuffer,
        event: wl_buffer::Event,
        index: &usize,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_buffer::Event::Release = event
            && let Some(slot) = state.buffers.slots.get_mut(*index)
        {
            slot.busy = false;
        }
    }
}
