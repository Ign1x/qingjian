//! [`super::Session`] 要 Server 做的事。真实现是 [`super::Link`]，测试里换成脚本。

use std::time::Instant;

use qingjian_platform::protocol::KeyEvent;
use qingjian_platform::protocol::linux::client::{ClientError, Reply};
use qingjian_platform::protocol::linux::{AppSurrounding, Capabilities, DisplayIdentity};

pub trait Server {
    /// 接下来的请求发给哪个应用的会话：一个应用一个会话，`[apps]` 按应用的设置与中 / 英状态都跟着会话走。
    /// `None` 是不知道哪个应用（合成器不报活动窗口）。会话第一次用到时才开。
    fn select(&mut self, app: Option<&str>);

    /// 输入框的能力（密码框 / 敏感），报给当前应用的会话。那个会话上次报的一样就不发；会话还没开就先记下，开的时候先报。
    fn capabilities(&mut self, capabilities: Capabilities) -> Result<(), ClientError>;

    /// 焦点进出。
    fn focus(&mut self, focused: bool) -> Result<Reply, ClientError>;

    /// 输入框没了：失焦后已经不能往原来的框里上屏，组句由 Server 丢弃。
    fn deactivate(&mut self) -> Result<Reply, ClientError>;

    /// 丢掉组句，不上屏。
    fn reset(&mut self) -> Result<Reply, ClientError>;

    /// 一次按键。组句中放行带 Ctrl / Alt / Super 的键时先把原文上屏（回包里带着），见 `Connection::key_flushing`。
    fn key(
        &mut self,
        event: KeyEvent,
        release: bool,
        surrounding: Option<AppSurrounding>,
    ) -> Result<Reply, ClientError>;

    /// 组句期间取最新的帧。
    fn poll(&mut self) -> Result<Reply, ClientError>;

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
}
