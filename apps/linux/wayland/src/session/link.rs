//! 与 Server 的那一条连接：懒连、断了退避重连、重连后补报能力与焦点。

use std::path::PathBuf;
use std::time::{Duration, Instant};

use qingjian_platform::protocol::linux::client::{ClientError, Connection, Reply};
use qingjian_platform::protocol::linux::{AppSurrounding, Capabilities, DisplayIdentity};
use qingjian_platform::protocol::{KeyEvent, SessionId};

use super::Server;

/// 这个前端在 Server 上的上下文标识（`LinuxHello.context`）：一个进程只开一个会话。
const CONTEXT: &str = "wayland";

/// 连不上之后多久再试。Server 主线程卡住时连接照样能建、每个请求却要等满 200 毫秒超时，不退避的话每个键都卡。
const BACKOFF: Duration = Duration::from_secs(2);

/// 一条连接与它上面的唯一会话。
pub struct Link {
    /// Server 的 socket。
    socket: PathBuf,

    /// 连接与会话；还没连或刚断开时是 `None`。
    connection: Option<(Connection, SessionId)>,

    /// 最近报过的能力。重连后先补报：Server 在收到能力之前不收别的。
    capabilities: Capabilities,

    /// 焦点在我们的输入框里：重连后补报 `Focus{true}`。
    focused: bool,

    /// 上次失败之后最早什么时候再试。
    retry_at: Option<Instant>,
}

impl Link {
    pub fn new(socket: PathBuf) -> Self {
        Self {
            socket,
            connection: None,
            capabilities: Capabilities::default(),
            focused: false,
            retry_at: None,
        }
    }

    /// 在会话上做一次事情；没连着就先连（退避期内直接失败）。出错就扔掉连接：一问一答的协议上一次失败之后收发可能已经串位。
    fn with<T>(
        &mut self,
        action: impl FnOnce(&mut Connection, SessionId) -> Result<T, ClientError>,
    ) -> Result<T, ClientError> {
        if self.connection.is_none() {
            self.open()?;
        }
        let (connection, session) = self.connection.as_mut().expect("刚连上");
        action(connection, *session).inspect_err(|error| {
            tracing::warn!(%error, "与 Server 的往返失败，断开，{} 秒后再连", BACKOFF.as_secs());
            self.connection = None;
            self.retry_at = Some(Instant::now() + BACKOFF);
        })
    }

    fn open(&mut self) -> Result<(), ClientError> {
        if self.retry_at.is_some_and(|at| Instant::now() < at) {
            return Err(ClientError::Connect {
                path: self.socket.display().to_string(),
                source: std::io::ErrorKind::WouldBlock.into(),
            });
        }
        let opened = (|| {
            let mut connection = Connection::connect_at(&self.socket)?;
            let session = connection.open_session(CONTEXT)?;
            connection.capabilities(session, self.capabilities)?;
            if self.focused {
                connection.focus(session, true)?;
            }
            Ok((connection, session))
        })();
        match opened {
            Ok(opened) => {
                tracing::info!(socket = %self.socket.display(), "连上 Server");
                self.connection = Some(opened);
                self.retry_at = None;
                Ok(())
            }
            Err(error) => {
                self.retry_at = Some(Instant::now() + BACKOFF);
                Err(error)
            }
        }
    }
}

impl Server for Link {
    fn capabilities(&mut self, capabilities: Capabilities) -> Result<(), ClientError> {
        if self.capabilities == capabilities {
            return Ok(());
        }
        self.capabilities = capabilities;
        if self.connection.is_none() {
            return Ok(());
        }
        self.with(|connection, session| connection.capabilities(session, capabilities))
            .map(drop)
    }

    fn focus(&mut self, focused: bool) -> Result<Reply, ClientError> {
        if !focused {
            self.focused = false;
            if self.connection.is_none() {
                return Ok(Reply::default());
            }
        }
        let reply = self.with(|connection, session| connection.focus(session, focused))?;
        self.focused = focused;
        Ok(reply)
    }

    fn deactivate(&mut self) -> Result<Reply, ClientError> {
        if self.connection.is_none() {
            return Ok(Reply::default());
        }
        self.with(|connection, session| connection.deactivate(session, true, true, false))
    }

    fn reset(&mut self) -> Result<Reply, ClientError> {
        self.with(|connection, session| connection.reset(session))
    }

    fn key(
        &mut self,
        event: KeyEvent,
        release: bool,
        surrounding: Option<AppSurrounding>,
    ) -> Result<Reply, ClientError> {
        self.with(|connection, session| {
            connection.key_flushing(session, event, release, surrounding)
        })
    }

    fn poll(&mut self) -> Result<Reply, ClientError> {
        self.with(|connection, session| connection.poll(session))
    }

    fn acknowledge(
        &mut self,
        identity: DisplayIdentity,
        senses: Vec<(usize, usize)>,
    ) -> Result<(), ClientError> {
        self.with(|connection, session| connection.acknowledge(session, identity, senses))
    }

    fn connected(&self) -> bool {
        self.connection.is_some()
    }

    fn retry_at(&self) -> Option<Instant> {
        self.retry_at
    }
}
