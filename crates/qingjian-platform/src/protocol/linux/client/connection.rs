//! 与 Server 的一条 Unix socket 连接。
//!
//! 一个进程一条连接，进程里每个输入上下文在这条连接上开一个会话
//! （Server 侧按「连接代次 + 上下文」隔离，见 [`crate::protocol::linux`]）。IBus 前端与 Wayland 前端共用。
//!
//! 收发是同步的一问一答，与 Fcitx5 插件的 `apps/linux/fcitx5/src/ipc/connection.cpp` 同一套：
//! **哪些消息有回包是协议的一部分**——`LinuxEvent` 与 `OpenSession` 有，
//! `CloseSession` 与展示回执没有。读多了会卡住，读少了下一次答复会串位。

use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::{Value, json};

use super::{ClientError, Reply};
use crate::protocol::linux::{
    AppSurrounding, Capabilities, DisplayAcknowledged, DisplayIdentity, LINUX_UI_PROTOCOL,
    LinuxEvent, ModeCommand, socket_path,
};
use crate::protocol::{
    ClientMessage, KeyEvent, KeyOutcome, PROTOCOL_VERSION, SessionId, read_message, write_message,
};

/// 一次收发的截止时间，与 Fcitx5 插件的 200 毫秒一致：输入法卡在 IO 上比丢一次按键更糟，
/// 超时就当连接坏了断开，下一次按键重连。
const TIMEOUT: Duration = Duration::from_millis(200);

/// 连接代次，Server 靠它认出「重连之后的新一批会话」，旧连接残留的展示回执按它作废。
/// Server 要求大于 0，且一条连接上的会话代次必须一致，所以按连接取一次。
static GENERATION: AtomicU64 = AtomicU64::new(1);

/// 一条连接。
pub struct Connection {
    /// socket 本身。
    stream: UnixStream,

    /// 本连接的代次。
    generation: u64,

    /// 下一个会话编号（连接内局部，Server 会映射成全局的）。
    next_session: u64,
}

impl Connection {
    /// 连上 Server。
    ///
    /// 连上之后核对对端是不是同一个用户：socket 在 `$XDG_RUNTIME_DIR` 下本来就是私有的，
    /// 但路径可以被 `QINGJIAN_SOCKET` 换掉，所以与 Server 侧一样两头都查一次。
    pub fn connect() -> Result<Self, ClientError> {
        Self::connect_at(&socket_path())
    }

    /// 连指定路径上的 Server。测试用假 Server 时走这条，不去动进程级的环境变量。
    pub fn connect_at(path: &std::path::Path) -> Result<Self, ClientError> {
        let stream = UnixStream::connect(path).map_err(|source| ClientError::Connect {
            path: path.display().to_string(),
            source,
        })?;
        stream.set_read_timeout(Some(TIMEOUT)).ok();
        stream.set_write_timeout(Some(TIMEOUT)).ok();
        if !same_user(&stream) {
            return Err(ClientError::UntrustedPeer);
        }
        Ok(Self {
            stream,
            generation: GENERATION.fetch_add(1, Ordering::Relaxed),
            next_session: 1,
        })
    }

    /// 给一个输入上下文开会话：`OpenSession` 之后立刻握 `LinuxHello`。
    ///
    /// `context` 是这个上下文的标识（Server 要求非空、不超过 64 字节、同连接内不重复），
    /// IBus 用输入上下文的对象路径，Wayland 前端用应用 app_id 的哈希。
    /// `app` 是宿主应用的标识，与 Fcitx5 插件报的 program 名同一种（X11 是 WM_CLASS，Wayland 是 app_id）：
    /// Server 据此查 `[apps]` 按应用的设置，不知道是哪个应用就给 `None`。
    pub fn open_session(
        &mut self,
        context: &str,
        app: Option<&str>,
    ) -> Result<SessionId, ClientError> {
        let session = SessionId(self.next_session);
        self.next_session += 1;
        let open = serde_json::to_value(ClientMessage::OpenSession {
            session,
            app: app.map(str::to_owned),
            protocol: PROTOCOL_VERSION,
        })?;
        self.request(&open)?;
        let hello = json!({"LinuxHello": {
            "session": session,
            "context": context,
            "generation": self.generation,
            "version": LINUX_UI_PROTOCOL,
        }});
        let reply = self.request(&hello)?;
        if reply.get("LinuxHello").is_none() {
            return Err(ClientError::Unexpected(reply.to_string()));
        }
        Ok(session)
    }

    /// 报一次框架能力（这个输入框是不是密码框之类）。
    ///
    /// **必须在别的事件之前报**：Server 在收到它之前不接受按键（没有它就不知道该不该进隐私模式）。
    pub fn capabilities(
        &mut self,
        session: SessionId,
        capabilities: Capabilities,
    ) -> Result<Reply, ClientError> {
        self.event(session, LinuxEvent::Capabilities(capabilities))
    }

    /// 报一次按键。`release` 是抬键——中 / 英切换要认 Shift 的抬起，所以抬键也得报。
    ///
    /// `surrounding` 是光标两侧的文字（见 [`AppSurrounding::around`]），云联想拿它当上文；
    /// 拿不到或者是敏感输入框就给 `None`。
    ///
    /// `selection_supported` 说这个前端能不能读应用的选区：给 `true` 时，选区翻译快捷键的回包是
    /// [`Reply::selection_request`]，要读出选区用 [`Self::selection`] 回；读不了就给 `false`，Server 把那个键当普通键。
    pub fn key(
        &mut self,
        session: SessionId,
        event: KeyEvent,
        release: bool,
        surrounding: Option<AppSurrounding>,
        selection_supported: bool,
    ) -> Result<Reply, ClientError> {
        self.event(
            session,
            LinuxEvent::Key {
                event,
                release,
                surrounding,
                selection_supported,
            },
        )
    }

    /// 报一次按键，组句中的快捷键先冲洗：Server 放行一个带 Ctrl / Alt / Super 的按下、而 preedit 还有内容时，
    /// 先 `Deactivate` 把原文上屏、再 `Focus{true}`（Deactivate 把会话标成不活跃，焦点其实还在这个框里），
    /// 回包带着要上屏的原文，按键照旧放行。调用方要先上屏、再把键交给应用。
    ///
    /// 应用收到 Ctrl+F 这类键会先自己挪焦点，输入法之后才知道失焦，那时再交出 preedit 已经落到新框或丢掉了；
    /// 与 macOS 在应用处理快捷键前 `commitComposition` 的结果一致。参数同 [`Self::key`]。
    pub fn key_flushing(
        &mut self,
        session: SessionId,
        event: KeyEvent,
        release: bool,
        surrounding: Option<AppSurrounding>,
        selection_supported: bool,
    ) -> Result<Reply, ClientError> {
        let shortcut = !release && event.modifiers.has_command_key();
        let reply = self.key(session, event, release, surrounding, selection_supported)?;
        if !shortcut || reply.outcome != KeyOutcome::Passthrough || reply.frame.preedit.is_empty() {
            return Ok(reply);
        }
        let flushed = self.deactivate(session, false, false, false)?;
        self.focus(session, true)?;
        Ok(Reply {
            outcome: KeyOutcome::Passthrough,
            ..flushed
        })
    }

    /// 报一次焦点进出。
    pub fn focus(&mut self, session: SessionId, focused: bool) -> Result<Reply, ClientError> {
        self.event(session, LinuxEvent::Focus { focused })
    }

    /// 这个上下文不再活跃（焦点离开、切走输入法、能力变了）。
    ///
    /// 缓冲里的东西怎么处置由 Server 按这三个事实决定：`focus_out` 且 `client_preedit` 时不上屏
    /// （应用自己画着 preedit 的场合），`capability_changed` 时直接丢弃（进了密码框就不能留着）。
    pub fn deactivate(
        &mut self,
        session: SessionId,
        focus_out: bool,
        client_preedit: bool,
        capability_changed: bool,
    ) -> Result<Reply, ClientError> {
        self.event(
            session,
            LinuxEvent::Deactivate {
                focus_out,
                client_preedit,
                capability_changed,
            },
        )
    }

    /// 应用要求重置：丢掉组句，不上屏。
    pub fn reset(&mut self, session: SessionId) -> Result<Reply, ClientError> {
        self.event(session, LinuxEvent::Reset)
    }

    /// 回 Server 的选区请求（[`Reply::selection_request`]）：`text` 是应用里选中的文字，没有选区或读不到给空串。
    /// 回包是触发它的那一键的结果：开始翻译时吃掉、帧是「翻译中…」（`translation_pending`，接着轮询），
    /// 不翻译（空串、超过 500 字、请求过期）时放行。
    pub fn selection(
        &mut self,
        session: SessionId,
        request: u64,
        text: String,
    ) -> Result<Reply, ClientError> {
        self.event(session, LinuxEvent::Selection { request, text })
    }

    /// 点了当前页第 `index` 格（从 0 起）：上屏它。
    ///
    /// `identity` 原样用画这一页的那次回包上的；帧已经换了，Server 回 `Ignored`，这边读成什么都不做的缺省答复。
    pub fn candidate(
        &mut self,
        session: SessionId,
        identity: DisplayIdentity,
        index: usize,
    ) -> Result<Reply, ClientError> {
        self.event(session, LinuxEvent::Candidate { identity, index })
    }

    /// 点了翻页：`next` 为下一页，否则上一页。`identity` 的规矩同 [`Self::candidate`]。
    pub fn page(
        &mut self,
        session: SessionId,
        identity: DisplayIdentity,
        next: bool,
    ) -> Result<Reply, ClientError> {
        self.event(session, LinuxEvent::Page { identity, next })
    }

    /// 右键了当前页第 `index` 格（Linux 协议 v4 起）：回包的 [`Reply::menu`] 是这格能做的几项，空的就不弹菜单。
    /// 只是问一句，帧与展示身份都不变。`identity` 的规矩同 [`Self::candidate`]。
    pub fn menu(
        &mut self,
        session: SessionId,
        identity: DisplayIdentity,
        index: usize,
    ) -> Result<Reply, ClientError> {
        self.event(session, LinuxEvent::Menu { identity, index })
    }

    /// 右键菜单里点了一项（v4 起）：`action` 原样用 [`MenuItem::id`](crate::protocol::linux::MenuItem::id)，
    /// `identity` 与 `index` 同弹菜单时的那次。回包的帧带着做完的那句提示（`notice`，下一次按键清），候选按做完的结果重排。
    pub fn menu_pick(
        &mut self,
        session: SessionId,
        identity: DisplayIdentity,
        index: usize,
        action: u32,
    ) -> Result<Reply, ClientError> {
        self.event(
            session,
            LinuxEvent::MenuPick {
                identity,
                index,
                action,
            },
        )
    }

    /// 托盘 / 状态显示面上切模式（v4 起）。回包像按键那样落地：切了中 / 英时 `english` 有值（拿它弹提示），
    /// 组句中的拼音原样上屏在 `commit` 里；`mode` 是切完的模式。
    pub fn indicator(
        &mut self,
        session: SessionId,
        command: ModeCommand,
    ) -> Result<Reply, ClientError> {
        self.event(session, LinuxEvent::Indicator(command))
    }

    /// 组句期间取一次最新的帧：本地整句模型的重排结果从这里来（Server 不推送）。
    /// 回的是 `Update`，帧没变时版本号与上次相同。
    pub fn poll(&mut self, session: SessionId) -> Result<Reply, ClientError> {
        let message = serde_json::to_value(ClientMessage::Poll { session })?;
        Reply::from_value(self.request(&message)?)
    }

    /// 回报这一帧真正画出来的译词义项：`(候选在本页的位置, 义项编号)`，生词标记据此衰减。没有回包。
    ///
    /// `identity` 原样用那一帧回包上的；对不上当前帧的 Server 整条丢掉。一次最多 128 对，多了 Server 断开连接。
    pub fn acknowledge(
        &mut self,
        session: SessionId,
        identity: DisplayIdentity,
        senses: Vec<(usize, usize)>,
    ) -> Result<(), ClientError> {
        let message = DisplayAcknowledged {
            session,
            identity,
            senses,
        };
        self.notify(&json!({ "DisplayAcknowledged": message }))
    }

    /// 关掉一个会话。没有回包。
    pub fn close_session(&mut self, session: SessionId) -> Result<(), ClientError> {
        let message = serde_json::to_value(ClientMessage::CloseSession { session })?;
        self.notify(&message)
    }

    /// 发一个 Linux 事件，取回 Server 的处置。
    fn event(&mut self, session: SessionId, event: LinuxEvent) -> Result<Reply, ClientError> {
        let message = json!({"LinuxEvent": {"session": session, "event": event}});
        Reply::from_value(self.request(&message)?)
    }

    /// 发一条有回包的消息。
    fn request(&mut self, message: &Value) -> Result<Value, ClientError> {
        self.notify(message)?;
        read_message::<_, Value>(&mut self.stream)?.ok_or(ClientError::Closed)
    }

    /// 发一条没有回包的消息。
    fn notify(&mut self, message: &Value) -> Result<(), ClientError> {
        write_message(&mut self.stream, message)?;
        Ok(())
    }
}

/// socket 对端是不是当前用户。
fn same_user(stream: &UnixStream) -> bool {
    let mut credentials = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut size = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: fd 来自还活着的 UnixStream，出参是本地的 ucred 与它的长度
    unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut credentials as *mut libc::ucred).cast(),
            &mut size,
        ) == 0
            && credentials.uid == libc::geteuid()
    }
}
