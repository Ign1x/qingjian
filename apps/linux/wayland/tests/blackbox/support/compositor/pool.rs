//! 客户端建的一个 wl_shm 池：它的匿名文件与当前大小，读缓冲里的像素时映射它。

use std::os::fd::OwnedFd;
use std::sync::atomic::{AtomicUsize, Ordering};

use memmap2::{Mmap, MmapOptions};

pub struct Pool {
    fd: OwnedFd,

    /// 字节数；客户端 resize 只会变大。
    size: AtomicUsize,
}

impl Pool {
    pub fn new(fd: OwnedFd, size: i32) -> Self {
        Self {
            fd,
            size: AtomicUsize::new(usize::try_from(size).unwrap_or_default()),
        }
    }

    pub fn resize(&self, size: i32) {
        self.size
            .store(usize::try_from(size).unwrap_or_default(), Ordering::Relaxed);
    }

    /// 只读映射整个池；映射不了是 `None`。
    pub fn map(&self) -> Option<Mmap> {
        let size = self.size.load(Ordering::Relaxed);
        // SAFETY: 只读映射客户端给的共享内存；只在缓冲挂上、还没还给客户端时读，那时客户端不往里写
        unsafe { MmapOptions::new().len(size).map(&self.fd) }.ok()
    }
}
