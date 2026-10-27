//! 核心协议里客户端要用的那几样：wl_compositor 与 wl_surface（候选窗那个 surface 每次 commit 记一笔）、
//! wl_region、wl_callback、wl_seat、wl_shm 与它的池和缓冲。

use std::sync::{Arc, Mutex, PoisonError};

use wayland_server::protocol::wl_buffer::{self, WlBuffer};
use wayland_server::protocol::wl_callback::{self, WlCallback};
use wayland_server::protocol::wl_compositor::{self, WlCompositor};
use wayland_server::protocol::wl_region::{self, WlRegion};
use wayland_server::protocol::wl_seat::{self, Capability, WlSeat};
use wayland_server::protocol::wl_shm::{self, Format, WlShm};
use wayland_server::protocol::wl_shm_pool::{self, WlShmPool};
use wayland_server::protocol::wl_surface::{self, WlSurface};
use wayland_server::{Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New};

use super::{Buffer, Pool, State, Surface};

impl GlobalDispatch<WlCompositor, ()> for State {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        resource: New<WlCompositor>,
        _: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<WlCompositor, ()> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &WlCompositor,
        request: wl_compositor::Request,
        _: &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_compositor::Request::CreateSurface { id } => {
                data_init.init(id, Mutex::new(Surface::default()));
            }
            wl_compositor::Request::CreateRegion { id } => {
                data_init.init(id, ());
            }
            _ => {}
        }
    }
}

impl Dispatch<WlSurface, Mutex<Surface>> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        resource: &WlSurface,
        request: wl_surface::Request,
        surface: &Mutex<Surface>,
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        let mut surface = surface.lock().unwrap_or_else(PoisonError::into_inner);
        match request {
            wl_surface::Request::Attach { buffer, .. } => surface.attach(buffer),
            wl_surface::Request::SetBufferScale { scale } => surface.scale = scale,
            wl_surface::Request::Frame { callback } => {
                data_init.init(callback, ()).done(0);
            }
            wl_surface::Request::Commit => {
                let committed = surface.commit();
                if state.popup.as_ref() == Some(resource) {
                    state.requests.push(committed);
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<WlRegion, ()> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &WlRegion,
        _: wl_region::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
    }
}

impl Dispatch<WlCallback, ()> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &WlCallback,
        _: wl_callback::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
    }
}

impl GlobalDispatch<WlSeat, ()> for State {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        resource: New<WlSeat>,
        _: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init
            .init(resource, ())
            .capabilities(Capability::Keyboard);
    }
}

/// 输入法不从 seat 拿键盘、指针，客户端不会发请求过来。
impl Dispatch<WlSeat, ()> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &WlSeat,
        _: wl_seat::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
    }
}

impl GlobalDispatch<WlShm, ()> for State {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        resource: New<WlShm>,
        _: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        let shm = data_init.init(resource, ());
        // 这两种是协议要求每个合成器都支持的
        shm.format(Format::Argb8888);
        shm.format(Format::Xrgb8888);
    }
}

impl Dispatch<WlShm, ()> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &WlShm,
        request: wl_shm::Request,
        _: &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let wl_shm::Request::CreatePool { id, fd, size } = request {
            data_init.init(id, Arc::new(Pool::new(fd, size)));
        }
    }
}

impl Dispatch<WlShmPool, Arc<Pool>> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &WlShmPool,
        request: wl_shm_pool::Request,
        pool: &Arc<Pool>,
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_shm_pool::Request::CreateBuffer {
                id,
                offset,
                width,
                height,
                stride,
                ..
            } => {
                data_init.init(id, Buffer::new(pool.clone(), offset, width, height, stride));
            }
            wl_shm_pool::Request::Resize { size } => pool.resize(size),
            _ => {}
        }
    }
}

impl Dispatch<WlBuffer, Buffer> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &WlBuffer,
        _: wl_buffer::Request,
        _: &Buffer,
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
    }
}
