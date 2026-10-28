//! [`super::Session`] 要 Server 做的事。真实现是 [`super::Link`]，测试里换成脚本。

use std::time::Instant;

use qingjian_platform::protocol::KeyEvent;
use qingjian_platform::protocol::linux::client::{ClientError, Reply};
use qingjian_platform::protocol::linux::{
    AppSurrounding, Capabilities, DisplayIdentity, MenuItem, ModeCommand,
};

pub trait Server {
    /// 接下来的请求发给哪个应用的会话：一个应用一个会话，`[apps]` 按应用的设置与中 / 英状态都跟着会话走。
    /// `None` 是不知道哪个应用（合成器不报活动窗口）。会话第一次用到时才开。
    fn select(&mut self, app: Option<&str>);

    /// 现在选着的应用。
    fn app(&self) -> Option<&str>;

    /// 输入框的能力（密码框 / 敏感），报给当前应用的会话。那个会话上次报的一样就不发；会话还没开就先记下，开的时候先报。
    fn capabilities(&mut self, capabilities: Capabilities) -> Result<(), ClientError>;

    /// 焦点进出。
    fn focus(&mut self, focused: bool) -> Result<Reply, ClientError>;

    /// 输入框没了：失焦后已经不能往原来的框里上屏，组句由 Server 丢弃。
    fn deactivate(&mut self) -> Result<Reply, ClientError>;

    /// 丢掉组句，不上屏。
    fn reset(&mut self) -> Result<Reply, ClientError>;

    /// 一次按键。组句中放行带 Ctrl / Alt / Super 的键时先把原文上屏（回包里带着），见 `Connection::key_flushing`。
    /// `selection_supported` 说这一键时应用里有能读的选区：报了的话选区翻译快捷键的回包是 [`Reply::selection_request`]，
    /// 要用 [`Self::selection`] 回了选区才有这一键的结果。
    fn key(
        &mut self,
        event: KeyEvent,
        release: bool,
        surrounding: Option<AppSurrounding>,
        selection_supported: bool,
    ) -> Result<Reply, ClientError>;

    /// 回 Server 的选区请求：`text` 是应用里选中的文字，读不了给空串。回包是触发它的那一键的结果。
    fn selection(&mut self, request: u64, text: String) -> Result<Reply, ClientError>;

    /// 组句期间取最新的帧。
    fn poll(&mut self) -> Result<Reply, ClientError>;

    /// 托盘上的切换（中 / 英、全角标点、简繁）。回包像按键那样落地：切中 / 英时组句原样上屏、带 `english`。
    fn indicator(&mut self, command: ModeCommand) -> Result<Reply, ClientError>;

    /// 回报画出来的译词义项。
    fn acknowledge(
        &mut self,
        identity: DisplayIdentity,
        senses: Vec<(usize, usize)>,
    ) -> Result<(), ClientError>;

    /// 现在连着（上一次往返没出错）。
    fn connected(&self) -> bool;

    /// 断开之后最早什么时候再试；没断过是 `None`。
    fn retry_at(&self) -> Option<Instant>;

    /// 点了候选窗上本页第 `index` 个候选。`identity` 是候选窗上那一帧的展示身份，帧已经换了 Server 回 `Ignored`。
    fn candidate(&mut self, identity: DisplayIdentity, index: usize) -> Result<Reply, ClientError>;

    /// 在候选窗上滚滚轮翻页：`next` 为下一页。`identity` 同 [`Self::candidate`]。
    fn page(&mut self, identity: DisplayIdentity, next: bool) -> Result<Reply, ClientError>;

    /// 右键了本页第 `index` 个候选：这一格能做的几项，空的就不弹菜单。只是问一句，帧与展示身份都不变。
    fn menu(
        &mut self,
        identity: DisplayIdentity,
        index: usize,
    ) -> Result<Vec<MenuItem>, ClientError>;

    /// 点了右键菜单里的一项：`index` 同弹菜单时的那一格，`action` 是那一项的 [`MenuItem::id`]。
    fn menu_pick(
        &mut self,
        identity: DisplayIdentity,
        index: usize,
        action: u32,
    ) -> Result<Reply, ClientError>;
}
