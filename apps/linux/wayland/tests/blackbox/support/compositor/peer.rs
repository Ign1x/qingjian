//! 被测进程那条连接的客户端数据：断开时记一笔，因为协议错误断开的连错误一起记。

use wayland_server::backend::{ClientData, ClientId, DisconnectReason};

use super::Request;
use crate::support::Log;

pub struct Peer {
    requests: Log<Request>,
}

impl Peer {
    pub fn new(requests: Log<Request>) -> Self {
        Self { requests }
    }
}

impl ClientData for Peer {
    fn disconnected(&self, _: ClientId, reason: DisconnectReason) {
        let error = match reason {
            DisconnectReason::ConnectionClosed => None,
            DisconnectReason::ProtocolError(error) => Some(format!("{error:?}")),
        };
        self.requests.push(Request::Disconnected(error));
    }
}
