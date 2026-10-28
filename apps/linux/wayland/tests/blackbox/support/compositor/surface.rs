//! 一个 surface 上 commit 才生效的状态：缓冲、viewport 的目标尺寸、收鼠标的区域、buffer scale。

use std::sync::{Mutex, PoisonError};

use wayland_server::Resource;
use wayland_server::protocol::wl_buffer::WlBuffer;

use super::Compositor;
use crate::support::compositor::buffer::Buffer;
use crate::support::compositor::picture::Picture;
use crate::support::compositor::request::Request;

pub struct Surface {
    /// 下次 commit 换上的缓冲：`Some(None)` 是挂空缓冲（收起）。
    attached: Option<Option<WlBuffer>>,

    /// 生效的画面；收起着是 `None`。
    picture: Option<Picture>,

    /// viewport 的目标尺寸（逻辑像素）；没设是 `None`。
    pub destination: Option<(i32, i32)>,

    /// 收鼠标的区域（surface 逻辑像素的矩形），最近一次 set_input_region 的拷贝。
    /// 协议上它要到 commit 才生效；这里设的时候记下就够（客户端设完马上 commit）。
    pub input_region: Vec<(i32, i32, i32, i32)>,

    pub scale: i32,

    hold: bool,

    held: Vec<WlBuffer>,
}

impl Default for Surface {
    fn default() -> Self {
        Self {
            attached: None,
            picture: None,
            destination: None,
            input_region: Vec::new(),
            scale: 1,
            hold: false,
            held: Vec::new(),
        }
    }
}

impl Surface {
    pub fn attach(&mut self, buffer: Option<WlBuffer>) {
        self.attached = Some(buffer);
    }

    /// commit：换上新挂的缓冲，缺省读出画面后马上还给客户端；测试可以暂扣以模拟缓冲耗尽，返回这次 commit 的样子。
    pub fn commit(&mut self) -> Request {
        if let Some(attached) = self.attached.take() {
            self.picture = attached.map(|buffer| {
                let picture = buffer.data::<Buffer>().map(Buffer::picture);
                if self.hold {
                    self.held.push(buffer);
                } else {
                    buffer.release();
                }
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

impl Compositor {
    /// 暂扣或归还候选窗缓冲，模拟合成器绘制慢于输入事件的情况。
    pub fn hold_buffers(&self, hold: bool) {
        self.call(move |state| {
            let popup = state.popup.as_ref().expect("候选窗已创建");
            let mut surface = popup
                .data::<Mutex<Surface>>()
                .expect("surface 状态")
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            surface.hold = hold;
            if !hold {
                for buffer in surface.held.drain(..) {
                    buffer.release();
                }
            }
        });
    }
}
