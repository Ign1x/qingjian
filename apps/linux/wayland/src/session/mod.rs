//! 输入法这一侧的全部判断：哪个键交给 Server、哪个原样转给应用，回包怎么落到应用里与候选窗上。
//!
//! 不碰 Wayland 对象，也不直接连 Server：两头各是一个 trait（[`Output`]、[`Server`]），
//! 真实现在 `crate::wayland` 与 [`Link`]，测试里换成记录器，所以这里的规则都能脱离合成器验。
//! 规则的来由见 `docs/design/linux-wayland.md`「协议对照」「按键」两节。

mod focus;
mod input;
mod keyboard;
mod keys;
mod link;
mod mouse;
mod output;
mod preedit;
mod repeat;
mod reply;
mod server;
mod unsent;

#[cfg(test)]
mod tests;

pub use input::{Click, Field, Input, Key, Modifiers, Spot, Surrounding};
pub use link::Link;
pub use output::Output;
pub use preedit::Preedit;
pub use server::Server;

use std::time::{Duration, Instant};

use qingjian_platform::protocol::Frame;
use qingjian_platform::protocol::linux::{AppSurrounding, DisplayIdentity};

use keys::Keys;
use mouse::Menu;
use repeat::Repeat;
use unsent::Unsent;

/// 组句期间多久问一次 Server 有没有新帧（重排结果、云端候选都靠它），与两个现有前端一样。
const POLL_INTERVAL: Duration = Duration::from_millis(80);

/// 发出一次文字更新之后，最多等多久应用的 commit（合成器转来的 `done`）再发下一次。
/// foot 这类应用收到 serial 对不上的 `done` 会把整次更新（连同上屏文字）丢掉，而它应用完我们的更新、
/// 重画后再 commit 要一帧左右；这期间发出去的下一次正好撞上。不 commit 的应用最多慢这么久。
const SETTLE: Duration = Duration::from_millis(80);

/// 输入框激活着时活动窗口换了应用，等多久再跟过去换 Server 上的会话。切换窗口时合成器报活动窗口与输入框失活、激活的先后不定，
/// 中间常有一瞬间没有激活着的窗口；等这一会儿，紧接着的失活、激活直接用上新应用，不用白换几次。
const FOLLOW: Duration = Duration::from_millis(200);

/// 一个输入法实例在一个 seat 上的状态。
pub struct Session<S> {
    server: S,

    /// 有输入框在要输入法。
    active: bool,

    /// 当前输入框的状态。
    field: Field,

    /// 键盘在我们手里。
    grabbed: bool,

    /// 按着的键。
    keys: Keys,

    /// 自己重复的那个键。
    repeat: Repeat,

    /// 最近一次给虚拟键盘的键位表；没有之前虚拟键盘上什么都不能发。
    keymap: Option<String>,

    /// 应用里该有的行内拼音（最新的一份，可能还攒在 `unsent` 里没发）。
    preedit: Preedit,

    /// 攒着、等应用落定再发的文字更新。
    unsent: Unsent,

    /// 上一次文字更新发出之后，最晚等到什么时候；`None` 是应用已经落定（收到了 `done`）。
    settle_at: Option<Instant>,

    /// 候选窗上现在画着的帧与它画出的义项；收着是 `None`。抬键的回包帧多半没变，没变就不重画。
    drawn: Option<(Frame, Vec<(usize, usize)>)>,

    /// 候选窗上那一帧的版本号；轮询回来版本号没变就不重画。
    shown: Option<u64>,

    /// 下一次轮询的时刻；没在组句是 `None`。
    poll_at: Option<Instant>,

    /// 活动窗口的应用（合成器报的最新一个）；合成器不报活动窗口时一直是 `None`，所有输入框共用一个会话。
    app: Option<String>,

    /// 什么时候看一眼 Server 上用着的是不是活动窗口那个应用的会话；输入框激活期间活动窗口换了应用才排上。
    follow_at: Option<Instant>,

    /// 候选窗上那一帧最近一次回包带的展示身份：点选、翻页、右键菜单原样带回去。
    /// 抬键的回包帧没变也换身份，Server 只认最新的那个。
    identity: Option<DisplayIdentity>,

    /// 弹着的候选右键菜单；收着是 `None`。
    menu: Option<Menu>,
}

impl<S: Server> Session<S> {
    pub fn new(server: S) -> Self {
        Self {
            server,
            active: false,
            field: Field::default(),
            grabbed: false,
            keys: Keys::default(),
            repeat: Repeat::default(),
            keymap: None,
            preedit: Preedit::default(),
            unsent: Unsent::default(),
            settle_at: None,
            drawn: None,
            shown: None,
            poll_at: None,
            app: None,
            follow_at: None,
            identity: None,
            menu: None,
        }
    }

    /// 处理排到的一件事。
    pub fn handle(&mut self, input: Input, out: &mut impl Output, now: Instant) {
        match input {
            Input::Activate(field) => self.activate(field, out),
            Input::Deactivate => self.deactivate(out),
            Input::Update(field) => self.update(field, out, now),
            Input::Keymap(keymap) => self.keymap(keymap, out),
            Input::Key(key) => self.key(key, out, now),
            Input::Modifiers(modifiers) => self.modifiers(modifiers, out),
            Input::Repeat { rate, delay } => self.repeat.configure(rate, delay),
            Input::App(app) => self.switch_app(app, now),
            Input::Click(click) => self.click(click, out, now),
            Input::Scroll { next, epoch } => self.scroll(next, epoch, out, now),
        }
    }

    /// 到点的事：按键重复、组句轮询、跟着活动窗口换会话、Server 断开后的重连；最后看看键盘该不该交还。
    /// 主循环每处理完一批事件调一次。
    pub fn tick(&mut self, out: &mut impl Output, now: Instant) {
        if self.settle_at.is_some_and(|at| at <= now) {
            self.settle_at = None;
            self.flush(out, now);
        }
        if let Some(key) = self.repeat.due(now) {
            self.repeat_key(key, out, now);
        }
        if self.poll_at.is_some_and(|at| at <= now) {
            self.poll_at = None;
            if self.serving() {
                self.poll(out, now);
            }
        }
        if self.follow_at.is_some_and(|at| at <= now) {
            self.follow_at = None;
            self.follow(out, now);
        }
        if self.reconnect_at().is_some_and(|at| at <= now) {
            self.connect(out);
        }
        self.yield_keyboard(out);
    }

    /// 下一件到点的事。
    pub fn deadline(&self) -> Option<Instant> {
        let settle = self.settle_at.filter(|_| self.unsent.dirty);
        [
            self.repeat.deadline(),
            self.poll_at,
            self.reconnect_at(),
            settle,
            self.follow_at,
        ]
        .into_iter()
        .flatten()
        .min()
    }

    /// 退出前：替转发过还按着的键补抬起（不然虚拟键盘上一直按着），交还键盘，收起候选窗。
    pub fn shutdown(&mut self, out: &mut impl Output) {
        self.flush(out, Instant::now());
        for code in self.keys.drain_forwarded() {
            out.forward(code, false, 0);
        }
        self.repeat.stop();
        if self.grabbed {
            out.grab(false);
            self.grabbed = false;
        }
        self.hide(out);
    }

    /// 键交给 Server 处理：有输入框、不是密码框、Server 连着。否则拿着键盘也只原样转发。
    fn serving(&self) -> bool {
        self.active && !self.field.password && self.server.connected()
    }

    /// Server 断开、而输入框还等着输入法时，什么时候再去连。
    fn reconnect_at(&self) -> Option<Instant> {
        if !self.active || self.field.password || self.grabbed || self.server.connected() {
            return None;
        }
        Some(self.server.retry_at().unwrap_or_else(Instant::now))
    }

    /// 不该再拿着键盘（没有输入框、密码框、Server 断了）而且没有按着的键，就交还给合成器。
    /// 还按着的键要等它们抬起：抬起经虚拟键盘转出去，虚拟键盘上才不会卡住。
    fn yield_keyboard(&mut self, out: &mut impl Output) {
        if self.grabbed && !self.serving() && self.keys.is_empty() {
            self.repeat.stop();
            out.grab(false);
            self.grabbed = false;
        }
    }

    /// 带给 Server 的光标前后文；敏感输入框不带。
    fn surrounding(&self) -> Option<AppSurrounding> {
        if self.field.sensitive {
            return None;
        }
        let surrounding = self.field.surrounding.as_ref()?;
        AppSurrounding::around(&surrounding.text, surrounding.cursor, surrounding.anchor)
    }

    /// 正在组句（应用里有拼音或候选窗开着）。
    fn composing(&self) -> bool {
        !self.preedit.text.is_empty() || self.drawn.is_some()
    }
}
