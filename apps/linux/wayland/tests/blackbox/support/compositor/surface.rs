//! 一个 surface 上 commit 才生效的状态：缓冲、viewport 的目标尺寸、buffer scale。

use wayland_server::Resource;
use wayland_server::protocol::wl_buffer::WlBuffer;

use super::{Buffer, Picture, Request};

pub struct Surface {
    /// 下次 commit 换上的缓冲：`Some(None)` 是挂空缓冲（收起）。
    attached: Option<Option<WlBuffer>>,

    /// 生效的画面；收起着是 `None`。
    picture: Option<Picture>,

    /// viewport 的目标尺寸（逻辑像素）；没设是 `None`。
    pub destination: Option<(i32, i32)>,

    pub scale: i32,
}

impl Default for Surface {
    fn default() -> Self {
        Self {
            attached: None,
            picture: None,
            destination: None,
            scale: 1,
        }
    }
}

impl Surface {
    pub fn attach(&mut self, buffer: Option<WlBuffer>) {
        self.attached = Some(buffer);
    }

    /// commit：换上新挂的缓冲，读出画面后马上还给客户端（像把它拷进纹理的合成器那样），返回这次 commit 的样子。
    pub fn commit(&mut self) -> Request {
        if let Some(attached) = self.attached.take() {
            self.picture = attached.map(|buffer| {
                let picture = buffer.data::<Buffer>().map(Buffer::picture);
                buffer.release();
                picture.expect("缓冲是 wl_shm 池建的")
            });
        }
        Request::Popup {
            picture: self.picture,
            destination: self.destination,
            scale: self.scale,
        }
    }
}
