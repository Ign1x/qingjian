//! 与 Server 的那一条连接：懒连、断了退避重连；一个应用一个会话，第一次用到时才开，开着的太多就关掉最久没用的。
//! 重连后只补开当前应用的会话、补报能力与焦点，别的应用等用到再开。

mod context;
mod opened;
mod sessions;

pub use context::context;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use qingjian_platform::protocol::linux::client::{ClientError, Connection, Reply};
use qingjian_platform::protocol::linux::{
    AppSurrounding, Capabilities, DisplayIdentity, MenuItem, ModeCommand,
};
use qingjian_platform::protocol::{KeyEvent, SessionId};

use opened::Opened;
use sessions::Sessions;

use super::Server;

/// 连不上之后多久再试。Server 主线程卡住时连接照样能建、每个请求却要等满 200 毫秒超时，不退避的话每个键都卡。
const BACKOFF: Duration = Duration::from_secs(2);

/// 一条连接与它上面各个应用的会话。
pub struct Link {
    /// Server 的 socket。
    socket: PathBuf,

    /// 还没连或刚断开时是 `None`。
    connection: Option<Connection>,

    /// 这条连接上开着的会话。
    sessions: Sessions,

    /// 接下来的请求发给哪个应用的会话；`None` 是不知道哪个应用。
    app: Option<String>,

    /// 当前输入框的能力：当前应用的会话开出来时先报它（Server 在收到能力之前不收别的）。
    capabilities: Capabilities,

    /// 焦点在当前应用的输入框里：它的会话重开时补报 `Focus{true}`。
    focused: bool,

    /// 上次失败之后最早什么时候再试。
    retry_at: Option<Instant>,

    /// 失败之后等多久再试。
    backoff: Duration,
}

impl Link {
    pub fn new(socket: PathBuf) -> Self {
        Self {
            socket,
            connection: None,
            sessions: Sessions::default(),
            app: None,
            capabilities: Capabilities::default(),
            focused: false,
            retry_at: None,
            backoff: BACKOFF,
        }
    }

    /// 测试里不等退避：断开之后下一次请求立刻重连。
    #[cfg(test)]
    pub fn without_backoff(mut self) -> Self {
        self.backoff = Duration::ZERO;
        self
    }

    /// 在当前应用的会话上做一次事情：没连着就先连、会话没开就先开（退避期内直接失败）。
    /// 出错就扔掉连接与上面全部的会话：一问一答的协议上一次失败之后收发可能已经串位。
    fn with<T>(
        &mut self,
        action: impl FnOnce(&mut Connection, SessionId) -> Result<T, ClientError>,
    ) -> Result<T, ClientError> {
        let established = self.connection.is_some();
        if !established && self.retry_at.is_some_and(|at| Instant::now() < at) {
            return Err(ClientError::Connect {
                path: self.socket.display().to_string(),
                source: std::io::ErrorKind::WouldBlock.into(),
            });
        }
        let result = self.session().and_then(|session| {
            let connection = self.connection.as_mut().expect("刚开好会话");
            action(connection, session)
        });
        if let Err(error) = &result {
            // 用着的连接断了才警告；Server 没起时每次退避到点都连不上，由会话层记 debug
            if established {
                tracing::warn!(%error, "与 Server 的往返失败，断开，{} 秒后再连", self.backoff.as_secs());
            }
            self.connection = None;
            self.sessions.clear();
            self.retry_at = Some(Instant::now() + self.backoff);
        }
        result
    }

    /// 当前应用的会话编号：没连着就先连，会话没开就先开。
    fn session(&mut self) -> Result<SessionId, ClientError> {
        if self.connection.is_none() {
            self.connection = Some(Connection::connect_at(&self.socket)?);
            let session = self.open()?;
            tracing::info!(socket = %self.socket.display(), "连上 Server");
            self.retry_at = None;
            return Ok(session);
        }
        match self.sessions.touch(self.app.as_deref()) {
            Some(session) => Ok(session),
            None => self.open(),
        }
    }

    /// 在这条连接上给当前应用开会话：先报能力（Server 在收到它之前不收别的），焦点在这里就补报焦点。
    fn open(&mut self) -> Result<SessionId, ClientError> {
        let connection = self.connection.as_mut().expect("连着才开会话");
        let context = context(self.app.as_deref());
        if let Some(evicted) = self.sessions.evict(&context) {
            connection.close_session(evicted)?;
        }
        let session = connection.open_session(&context, self.app.as_deref())?;
        connection.capabilities(session, self.capabilities)?;
        if self.focused {
            connection.focus(session, true)?;
        }
        self.sessions.add(Opened {
            app: self.app.clone(),
            context,
            id: session,
            capabilities: self.capabilities,
            used: 0,
        });
        Ok(session)
    }
}

impl Server for Link {
    fn select(&mut self, app: Option<&str>) {
        if self.app.as_deref() != app {
            self.app = app.map(str::to_owned);
            // 新应用的会话还没拿到焦点（旧应用的由会话层先报了失焦）
            self.focused = false;
        }
    }

    fn app(&self) -> Option<&str> {
        self.app.as_deref()
    }

    fn capabilities(&mut self, capabilities: Capabilities) -> Result<(), ClientError> {
        self.capabilities = capabilities;
        // 会话还没开的话开的时候先报；开着的只在变了时报：Server 收到不同的能力会丢掉组句
        let unchanged = match self.sessions.get(self.app.as_deref()) {
            Some(opened) => opened.capabilities == capabilities,
            None => return Ok(()),
        };
        if unchanged {
            return Ok(());
        }
        self.with(|connection, session| connection.capabilities(session, capabilities))?;
        if let Some(opened) = self.sessions.get_mut(self.app.as_deref()) {
            opened.capabilities = capabilities;
        }
        Ok(())
    }

    fn focus(&mut self, focused: bool) -> Result<Reply, ClientError> {
        if !focused {
            self.focused = false;
            // 会话没开（没连着、或这个应用还没用过）就没有要失焦的
            if self.sessions.get(self.app.as_deref()).is_none() {
                return Ok(Reply::default());
            }
        }
        let reply = self.with(|connection, session| connection.focus(session, focused))?;
        self.focused = focused;
        Ok(reply)
    }

    fn deactivate(&mut self) -> Result<Reply, ClientError> {
        if self.sessions.get(self.app.as_deref()).is_none() {
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
        selection_supported: bool,
    ) -> Result<Reply, ClientError> {
        self.with(|connection, session| {
            connection.key_flushing(session, event, release, surrounding, selection_supported)
        })
    }

    fn selection(&mut self, request: u64, text: String) -> Result<Reply, ClientError> {
        self.with(|connection, session| connection.selection(session, request, text))
    }

    fn poll(&mut self) -> Result<Reply, ClientError> {
        self.with(|connection, session| connection.poll(session))
    }

    fn indicator(&mut self, command: ModeCommand) -> Result<Reply, ClientError> {
        self.with(|connection, session| connection.indicator(session, command))
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

    fn candidate(&mut self, identity: DisplayIdentity, index: usize) -> Result<Reply, ClientError> {
        self.with(|connection, session| connection.candidate(session, identity, index))
    }

    fn page(&mut self, identity: DisplayIdentity, next: bool) -> Result<Reply, ClientError> {
        self.with(|connection, session| connection.page(session, identity, next))
    }

    fn menu(
        &mut self,
        identity: DisplayIdentity,
        index: usize,
    ) -> Result<Vec<MenuItem>, ClientError> {
        self.with(|connection, session| connection.menu(session, identity, index))
    }

    fn menu_pick(
        &mut self,
        identity: DisplayIdentity,
        index: usize,
        action: u32,
    ) -> Result<Reply, ClientError> {
        self.with(|connection, session| connection.menu_pick(session, identity, index, action))
    }
}
